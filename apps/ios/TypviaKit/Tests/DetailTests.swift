// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// The rules of one snippet's screen: what can be saved, what a secret is
/// allowed to show and for how long, and how a template's blanks are found.
final class DetailTests: XCTestCase {
    // MARK: - What can be saved

    func testABodyIsWhatMakesADraftSaveable() {
        XCTAssertFalse(Draft().canSave)
        XCTAssertFalse(Draft(title: "A name for nothing").canSave)
        XCTAssertFalse(Draft(body: "   \n  ").canSave, "whitespace is not content")
        XCTAssertTrue(Draft(body: "kubectl get pods").canSave)
    }

    func testANewDraftNeedsNoKindChosenUpFront() {
        let draft = Draft(body: "something")
        XCTAssertNil(draft.sort, "the reader is not made to file it before writing it")
        XCTAssertEqual(draft.effectiveSort, .text)
    }

    /// The screen hands over what was typed, trimmed. Naming an untitled
    /// snippet after its first line moved into the shared layer — this used to
    /// be the only place it lived, which is how the other platform ended up
    /// refusing the same empty field this one quietly filled in.
    func testTheScreenHandsOverTheNameAsTyped() {
        XCTAssertEqual(DetailModel.name(for: Draft(title: "  Deploy  ", body: "./deploy.sh")), "Deploy")
        XCTAssertEqual(DetailModel.name(for: Draft(body: "kubectl get pods")), "")
    }

    func testAnEmptyTriggerIsNoTriggerRatherThanAnEmptyOne() {
        XCTAssertNil(DetailModel.trigger(for: Draft(body: "x", trigger: "   ")))
        XCTAssertEqual(DetailModel.trigger(for: Draft(body: "x", trigger: " ;dep ")), ";dep")
    }

    // MARK: - Refusals belong under the field

    /// The kind crosses; the words do not. The engine writes in English for
    /// whoever reads a log, and this product renders one language at a time.
    func testARefusalCarriesItsKindAndNeverTheEnginesWords() {
        XCTAssertEqual(Refusal(.Conflict(reason: "that trigger is taken")), .clash)
        XCTAssertEqual(Refusal(.Validation(reason: "too long")), .invalid)
        XCTAssertEqual(Refusal(.PermissionDenied(reason: "unlock first")), .notPermitted)
        XCTAssertEqual(Refusal(.RuleBlocked(reason: "blocked for Safari")), .ruleBlocked)
        XCTAssertEqual(Refusal(.Unavailable(reason: "server unreachable")), .offline)
    }

    func testAStorageFailureIsNamedRatherThanQuoted() {
        // The underlying message can contain paths and SQL.
        XCTAssertEqual(Refusal(.System), .storage)
        XCTAssertEqual(Refusal(.NotFound), .missing)
    }

    /// Every kind has a sentence in both languages, and the two halves are
    /// different sentences. A missing half is how a single-language screen
    /// turns into a mixed one.
    func testEveryRefusalIsWrittenInBothLanguages() {
        let kinds: [Refusal] = [
            .invalid, .clash, .notPermitted, .ruleBlocked, .offline, .missing, .storage,
        ]
        let en = Translator(language: .en)
        let zh = Translator(language: .zh)
        for kind in kinds {
            XCTAssertFalse(kind.sentence(en).isEmpty)
            XCTAssertFalse(kind.sentence(zh).isEmpty)
            XCTAssertNotEqual(kind.sentence(en), kind.sentence(zh))
        }
    }

    // MARK: - A secret, out and back

    func testTheWindowShrinksToNothingAndNeverPastIt() {
        XCTAssertEqual(Reveal.remaining(secondsLeft: Reveal.window), 1, accuracy: 0.0001)
        XCTAssertEqual(Reveal.remaining(secondsLeft: Reveal.window / 2), 0.5, accuracy: 0.0001)
        XCTAssertEqual(Reveal.remaining(secondsLeft: 0), 0, accuracy: 0.0001)
        XCTAssertEqual(Reveal.remaining(secondsLeft: -5), 0, accuracy: 0.0001)
        XCTAssertEqual(Reveal.remaining(secondsLeft: 999), 1, accuracy: 0.0001)
    }

    func testTheRevealStateCarriesNoPlaintext() {
        // The state machine outlives a frame; a decrypted secret must not.
        // This is a shape assertion: `Reveal` has no case with a body.
        let shown = Reveal.shown(secondsLeft: 3)
        XCTAssertNotEqual(shown, .hidden)
        XCTAssertEqual(Reveal.shown(secondsLeft: 3), shown)
    }

    // MARK: - The clipboard offers, briefly

    func testOnlyAFreshCopyIsOffered() {
        XCTAssertTrue(ClipboardOffer(text: "x", age: 0).isFresh)
        XCTAssertTrue(ClipboardOffer(text: "x", age: 59).isFresh)
        XCTAssertFalse(ClipboardOffer(text: "x", age: 61).isFresh, "an hour-old clipboard is a surprise")
        XCTAssertFalse(ClipboardOffer(text: "x", age: -1).isFresh)
    }

    // MARK: - A template's blanks

    func testTheBlanksAreFoundInTheEnginesOwnMarkers() {
        let body = TemplateBody(preview: "各位,‹服务名›将在‹时间›上线。")
        XCTAssertEqual(body.blanks, ["服务名", "时间"])
        XCTAssertEqual(
            body.pieces,
            [.text("各位,"), .blank("服务名"), .text("将在"), .blank("时间"), .text("上线。")]
        )
    }

    func testATemplateWithNoBlanksIsJustWords() {
        let body = TemplateBody(preview: "nothing to fill in")
        XCTAssertTrue(body.blanks.isEmpty)
        XCTAssertEqual(body.pieces, [.text("nothing to fill in")])
    }

    func testAnUnclosedMarkerStaysTextRatherThanBecomingAField() {
        let body = TemplateBody(preview: "half ‹written")
        XCTAssertTrue(body.blanks.isEmpty)
        XCTAssertEqual(body.pieces, [.text("half ‹written")])
    }

    func testABlankAtEitherEndSurvives() {
        XCTAssertEqual(TemplateBody(preview: "‹name› trails").blanks, ["name"])
        XCTAssertEqual(TemplateBody(preview: "leads ‹name›").blanks, ["name"])
    }
}

/// Filling a template, against a real database and the shared engine.
///
/// What is under test is that the filling goes through the engine — Swift
/// holds no substitution rule of its own — and that a missing required field
/// is the engine's refusal rather than a local guess.
@MainActor
final class TemplateFillTests: XCTestCase {
    private var directory: URL!
    private var store: TypviaStore!

    override func setUpWithError() throws {
        directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
        store = try TypviaStore(dataDirectory: directory)
    }

    override func tearDownWithError() throws {
        store = nil
        try? FileManager.default.removeItem(at: directory)
    }

    func testATemplatesBlanksComeFromTheEngineAndSoDoesTheFilledText() async throws {
        let created = try await store.perform {
            try $0.snippetCreate(
                draft: SnippetDraft(
                    title: "上线通知",
                    body: "各位,{{service}} 将在 {{when}} 上线。",
                    snippetType: "template",
                    description: nil, folderId: nil, trigger: ";ship",
                    triggerMode: "delimiter", language: nil
                )
            )
        }
        let model = DetailModel(store: store, snippetId: created.id)
        await model.load()

        XCTAssertEqual(model.fields.map(\.name), ["service", "when"])
        XCTAssertEqual(model.templateBody?.blanks, ["service", "when"])

        model.values = ["service": "支付网关", "when": "周四 20:00"]
        let filled = await model.filled()
        XCTAssertEqual(filled, "各位,支付网关 将在 周四 20:00 上线。")
    }

    func testAMissingFieldIsTheEnginesRefusalNotALocalGuess() async throws {
        let created = try await store.perform {
            try $0.snippetCreate(
                draft: SnippetDraft(
                    title: "上线通知", body: "各位,{{service}} 将在 {{when}} 上线。",
                    snippetType: "template", description: nil, folderId: nil,
                    trigger: nil, triggerMode: nil, language: nil
                )
            )
        }
        let model = DetailModel(store: store, snippetId: created.id)
        await model.load()
        model.values = ["service": "支付网关"]

        let filled = await model.filled()
        XCTAssertNil(filled, "an unfilled required blank must not silently render as empty")
        XCTAssertNotNil(model.refusal, "and the reader is told, on the line under the fields")
    }

    func testANonTemplateHasNoBlanksAtAll() async throws {
        let created = try await store.perform {
            try $0.snippetCreate(
                draft: SnippetDraft(
                    title: "Plain", body: "no blanks {{here}}", snippetType: "text",
                    description: nil, folderId: nil, trigger: nil, triggerMode: nil, language: nil
                )
            )
        }
        let model = DetailModel(store: store, snippetId: created.id)
        await model.load()
        XCTAssertTrue(model.fields.isEmpty)
        XCTAssertNil(model.templateBody)
    }
}

/// Filing something in a folder that does not exist yet.
///
/// The bridge could make folders from the start and nothing on this platform
/// ever called it, so the editor could only offer folders made somewhere else
/// — and the row was hidden entirely when there were none, which on a
/// phone-only library is always.
@MainActor
final class FolderMakingTests: XCTestCase {
    private var directory: URL!
    private var store: TypviaStore!

    override func setUpWithError() throws {
        directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
        store = try TypviaStore(dataDirectory: directory)
    }

    override func tearDownWithError() throws {
        store = nil
        try? FileManager.default.removeItem(at: directory)
    }

    func testANewFolderIsMadeAndTheDraftGoesStraightIntoIt() async throws {
        let model = DetailModel(store: store)
        await model.load()
        XCTAssertTrue(model.folders.isEmpty)

        let made = await model.makeFolder(named: "上线")

        XCTAssertTrue(made)
        XCTAssertEqual(model.folders.map(\.name), ["上线"])
        XCTAssertEqual(model.draft.folderId, model.folders.first?.id)
        XCTAssertNil(model.refusal)
    }

    /// The rule is the core's — the same one the desktop is held to — and the
    /// screen reports it rather than deciding it.
    func testANamelessFolderIsRefusedAndNothingIsMade() async throws {
        let model = DetailModel(store: store)
        await model.load()

        let made = await model.makeFolder(named: "   ")

        XCTAssertFalse(made)
        XCTAssertEqual(model.refusal, .invalid)
        XCTAssertTrue(model.folders.isEmpty)
        XCTAssertNil(model.draft.folderId)
    }

    func testTheNameIsStoredTheWayAReaderWouldWriteIt() async throws {
        let model = DetailModel(store: store)
        await model.load()

        _ = await model.makeFolder(named: "  上线  ")

        XCTAssertEqual(model.folders.map(\.name), ["上线"], "one folder to a reader, one row here")
    }

    func testASnippetSavedIntoANewFolderIsFiledThere() async throws {
        let model = DetailModel(store: store)
        await model.load()
        model.draft = Draft(title: "Rollback", body: "kubectl rollout undo deploy/api")
        _ = await model.makeFolder(named: "上线")

        let saved = await model.save()

        XCTAssertTrue(saved)
        XCTAssertEqual(model.snippet?.folderId, model.folders.first?.id)
    }
}

/// The AI strip. The rule that matters most is the one this screen does not
/// enforce: a vault snippet is refused behind the bridge, before a prompt
/// exists. What is tested here is that the screen never gets in the way of
/// that, and never acts on its own.
@MainActor
final class AiStripTests: XCTestCase {
    private var directory: URL!
    private var store: TypviaStore!

    override func setUpWithError() throws {
        directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
        store = try TypviaStore(dataDirectory: directory)
    }

    override func tearDownWithError() throws {
        store = nil
        try? FileManager.default.removeItem(at: directory)
    }

    func testWithNoEngineTheStripSaysWhereToSetOneUpRatherThanFailing() async {
        let model = DetailModel(store: store)
        model.draft.body = "some text"
        await model.organize()
        XCTAssertEqual(model.ai, .noEngine, "not configured is not the same as broken")
    }

    func testOnlyActionsWhoseContractAcceptsASnippetBodyAreOffered() async {
        let model = DetailModel(store: store)
        await model.loadActions()
        for action in model.actions {
            XCTAssertTrue(
                action.canRunHere,
                "\(action.name) asks for \(action.inputSource); this screen has a snippet body"
            )
        }
    }

    func testASuggestionProposingNothingIsNotShown() {
        let empty = AiSuggestion(
            AiOrganizeSuggestion(
                title: nil, description: nil, snippetType: nil, securityLevel: nil,
                trigger: nil, tags: [], folder: nil
            )
        )
        XCTAssertTrue(empty.isEmpty, "a strip that lights up for nothing teaches people to ignore it")
    }

    func testNothingIsAppliedUntilTheReaderSaysSo() {
        let model = DetailModel(store: store)
        model.draft = Draft(title: "mine", body: "text", trigger: ";mine")
        let suggestion = AiSuggestion(
            AiOrganizeSuggestion(
                title: "theirs", description: nil, snippetType: "code",
                securityLevel: nil, trigger: ";theirs", tags: [], folder: nil
            )
        )
        // Proposed, not applied.
        model.dismissAi()
        XCTAssertEqual(model.draft.title, "mine")
        XCTAssertEqual(model.draft.trigger, ";mine")

        model.accept(suggestion)
        XCTAssertEqual(model.draft.title, "theirs")
        XCTAssertEqual(model.draft.sort, .code)
        XCTAssertEqual(model.draft.trigger, ";theirs")
    }

    func testAcceptingTakesOnlyWhatWasActuallyProposed() {
        let model = DetailModel(store: store)
        model.draft = Draft(title: "mine", body: "text", trigger: ";mine")
        model.accept(
            AiSuggestion(
                AiOrganizeSuggestion(
                    title: nil, description: nil, snippetType: nil, securityLevel: nil,
                    trigger: nil, tags: [], folder: nil
                )
            )
        )
        XCTAssertEqual(model.draft.title, "mine", "an absent proposal must not blank a field")
        XCTAssertEqual(model.draft.trigger, ";mine")
    }
}

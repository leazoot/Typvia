// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI
import UIKit
import XCTest

@testable import TypviaKit

/// Pairing and first launch. The rules worth pinning are about what a reader
/// is asked to compare, and about a code that must not change while they are
/// reading it out.
final class OnboardingTests: XCTestCase {
    // MARK: - The code

    func testACodeIsGroupedSoItCanBeReadAloud() {
        XCTAssertEqual(PairingCode.grouped("ABCD1234EFGH"), "ABCD 1234 EFGH")
        XCTAssertEqual(PairingCode.grouped("ABCDE", every: 2), "AB CD E")
        XCTAssertEqual(PairingCode.grouped(""), "")
    }

    func testAnExpiredCodeSaysSoRatherThanRefreshingItself() {
        let live = PairingCode(code: "ABCD", secondsLeft: 240)
        XCTAssertFalse(live.hasExpired)
        XCTAssertEqual(live.minutesLeft, 4)

        let dead = PairingCode(code: "ABCD", secondsLeft: 0)
        XCTAssertTrue(dead.hasExpired)
        XCTAssertEqual(dead.minutesLeft, 0, "a code that swapped itself out mid-sentence would not work")
    }

    /// A WebDAV folder has no session server, so nobody promised a window. A
    /// code with no window is not expired — it was declared dead the moment it
    /// appeared, because "no window" had been folded into "zero seconds left".
    func testACodeNobodyPromisedAWindowForHasNotExpired() {
        let noWindow = PairingCode(code: "ABCD", secondsLeft: nil)

        XCTAssertFalse(noWindow.hasExpired, "nothing was undertaken to run out")
        XCTAssertEqual(noWindow.minutesLeft, 0, "and no window is stated either")
    }

    func testTheMinutesRoundUpSoTheLastPartMinuteIsNotReportedAsZero() {
        XCTAssertEqual(PairingCode(code: "A", secondsLeft: 61).minutesLeft, 2)
        XCTAssertEqual(PairingCode(code: "A", secondsLeft: 1).minutesLeft, 1)
    }

    // MARK: - The check string

    func testTheCharactersArriveOneAtATimeAndTheLineKeepsPace() {
        let none = SasReveal(sas: Self.sas, shown: 0)
        XCTAssertEqual(none.progress, 0, accuracy: 0.0001)
        XCTAssertFalse(none.isComplete)

        let half = SasReveal(sas: Self.sas, shown: 10)
        XCTAssertEqual(half.progress, 0.5, accuracy: 0.0001)

        let all = SasReveal(sas: Self.sas, shown: 20)
        XCTAssertTrue(all.isComplete)
        XCTAssertEqual(all.progress, 1, accuracy: 0.0001)
    }

    func testTheRevealNeverRunsPastTheCharactersItHas() {
        XCTAssertEqual(SasReveal(sas: Self.sas, shown: 99).shown, 20)
        XCTAssertEqual(SasReveal(sas: Self.sas, shown: -3).shown, 0)
        XCTAssertFalse(SasReveal(sas: "", shown: 3).isComplete, "no characters is not a completed check")
    }

    /// The separators are the core's way of grouping for reading aloud, not
    /// something either reader compares. Counting them would beat the reveal on
    /// a hyphen and leave the line under the characters behind them.
    func testTheSeparatorsAreNotCharactersToCompare() {
        XCTAssertEqual(SasReveal(sas: Self.sas, shown: 0).characters.count, 20)
        XCTAssertFalse(
            SasReveal(sas: Self.sas, shown: 20).characters.contains("-"),
            "a separator is not one of the characters"
        )
        XCTAssertEqual(SasReveal(sas: Self.sas, shown: 10).spoken, "ABCDE FGHIJ")
        XCTAssertEqual(SasReveal(sas: Self.sas, shown: 0).spoken, "")
    }

    /// The one the walkthrough caught: the page was written for a six-character
    /// check and the core hands over four groups of five, so a single line of
    /// them ran off the edge of the phone and the reader was asked whether a
    /// string they could only half see matched.
    func testTheWholeStringIsLaidOutInRowsThatFit() {
        let rows = SasReveal(sas: Self.sas, shown: 20).rows()

        XCTAssertEqual(rows.flatMap { $0 }.reduce(0) { $0 + $1.characters.count }, 20)
        for row in rows {
            XCTAssertLessThanOrEqual(
                row.reduce(0) { $0 + $1.characters.count },
                SasReveal.charactersPerRow,
                "a row wider than the narrowest phone is a check nobody can make"
            )
            // Groups stay whole: half a group on each line is how a comparison
            // is read wrong.
            XCTAssertTrue(row.allSatisfy { $0.characters.count == 5 })
        }
        XCTAssertEqual(rows.first?.map(\.text), ["ABCDE", "FGHIJ"])
    }

    /// Characters that have not landed hold their place, so the string does not
    /// crawl sideways as the rest of it arrives.
    func testAGroupKeepsItsWidthWhileItIsStillArriving() {
        let rows = SasReveal(sas: Self.sas, shown: 12).rows()

        XCTAssertEqual(rows[0].map(\.text), ["ABCDE", "FGHIJ"])
        XCTAssertEqual(rows[1].map(\.text), ["KL   ", "     "])
    }

    /// The claim the row width rests on, measured rather than asserted: a full
    /// row set at the code rung has to fit the narrowest phone this app runs
    /// on, at the largest Dynamic Type step it honours. A row that does not fit
    /// is the defect this whole layout exists to undo.
    func testAFullRowFitsTheNarrowestPhoneAtEveryTypeStepItHonours() {
        // iPhone SE (2nd generation) is the narrowest screen iOS 16 runs on.
        let available = 375 - 2 * Tokens.Space.screenPadding

        XCTAssertLessThanOrEqual(
            Self.rowWidth(characters: SasReveal.charactersPerRow, at: .xxxLarge),
            available,
            "a row that runs off the edge cannot be compared"
        )
        XCTAssertLessThanOrEqual(
            Self.rowWidth(characters: SasReveal.charactersPerStackedRow, at: TypviaType.ceiling),
            available,
            "the accessibility steps are where the wide row stopped fitting"
        )
    }

    /// A row of that many characters set at the code rung, plus the gap that
    /// separates the groups in it.
    private static func rowWidth(characters: Int, at typeSize: DynamicTypeSize) -> CGFloat {
        let row = String(repeating: "M", count: characters)
        let attributes: [NSAttributedString.Key: Any] = [
            .font: TypviaType.code.uiFont(at: typeSize),
            .kern: TypviaType.code.tracking(at: typeSize),
        ]
        return (row as NSString).size(withAttributes: attributes).width
            + OnboardingMetrics.sasGroupGap
    }

    func testTheStepsSpreadAcrossTheDeliverysWindow() {
        XCTAssertEqual(SasReveal.interval(for: 6), 0.120, accuracy: 0.0001)
        XCTAssertEqual(SasReveal.interval(for: 0), 0.720, accuracy: 0.0001)
    }

    /// The shape the core actually hands over: four groups of five.
    private static let sas = "ABCDE-FGHIJ-KLMNO-PQRST"

    func testWhichNumberedStepEachPairingStateIs() {
        XCTAssertEqual(PairingStep.offering(code: "A", expiresAt: 0).number, 1)
        XCTAssertEqual(PairingStep.dropped.number, 1, "a dropped pairing is back at the code")
        XCTAssertEqual(PairingStep.comparing(sas: "K7M2Q4", otherDevice: "Mac").number, 2)
        XCTAssertEqual(PairingStep.joined.number, 2)
    }

    // MARK: - First launch

    func testTheFirstRunIsThreeScreensAndTwoOfThemDemonstrateTheProduct() {
        XCTAssertEqual(FirstRun.total, 3)
        XCTAssertNotNil(FirstRun.save.demonstration)
        XCTAssertNotNil(FirstRun.call.demonstration)
        XCTAssertNil(FirstRun.keyboard.demonstration, "the last screen is instructions, not a demo")
    }

    /// The red line this product's keyboard actually holds: it declares that
    /// it does not want full access, so iOS draws no such switch. Every place
    /// that told the reader to turn one on was sending them to look for a
    /// control that is not there — and, in the panel's case, was diagnosing an
    /// empty bench with the one cause it could not possibly be.
    func testNoSurfaceAsksForAPermissionThisKeyboardDoesNotWant() {
        for language in UiLanguage.allCases {
            let tr = Translator(language: language)
            let said = ([
                KeyboardPanelCopy.nothingToRead(tr),
                KeyboardPanelCopy.openTheAppOnce(tr),
                FirstRunKeyboardCopy.intro(tr),
            ] + FirstRunKeyboardCopy.steps(tr)).joined(separator: " ")
            XCTAssertFalse(
                said.contains("Turn on full access") || said.contains("打开「完全访问权限」"),
                "a surface is still asking for a switch iOS does not draw: \(said)"
            )
        }
    }

    /// The step that could not be carried out is gone rather than reworded.
    func testTheKeyboardErrandIsTwoStepsBecauseTheThirdWasImpossible() {
        for language in UiLanguage.allCases {
            XCTAssertEqual(FirstRunKeyboardCopy.steps(Translator(language: language)).count, 2)
        }
    }

    /// What the shell reads on the way back into the app. An unreadable list
    /// must not be reported as a keyboard that is there.
    func testTheKeyboardListIsReadFromTheSystemAndSilenceIsNotAYes() {
        let empty = UserDefaults(suiteName: "typvia.tests.keyboards.\(UUID().uuidString)")
        XCTAssertEqual(KeyboardInstall.current(defaults: empty ?? .standard), .notAdded)

        let added = UserDefaults(suiteName: "typvia.tests.keyboards.\(UUID().uuidString)")
        added?.set(["com.apple.keyboard.emoji", KeyboardInstall.bundleId], forKey: KeyboardInstall.installedKeyboardsKey)
        XCTAssertEqual(KeyboardInstall.current(defaults: added ?? .standard), .ready)
    }

    func testWhetherTheKeyboardWasAddedIsCheckedRatherThanAssumed() {
        XCTAssertEqual(KeyboardInstall.state(activeInputModes: []), .notAdded)
        XCTAssertEqual(
            KeyboardInstall.state(activeInputModes: ["com.apple.keyboard.emoji"]),
            .notAdded
        )
        XCTAssertEqual(
            KeyboardInstall.state(activeInputModes: ["dev.typvia.mobile.keyboard"]),
            .ready
        )
    }
}

/// Joining an account from this device.
///
/// The seven pairing methods existed from the start with nothing calling them,
/// and this side of the exchange is the one a phone actually needs: a
/// phone-only library cannot admit anybody, because it has no account to admit
/// them to. What can be tested without a server is the part that decides —
/// what is enough to ask with, what leaves the screen holding a password, and
/// that nothing claims to have joined when it has not.
@MainActor
final class PairingJoinTests: XCTestCase {
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

    func testAServerNeedsBothItsAddressAndAnAccount() {
        let model = PairingModel(store: store)
        model.target = .server
        XCTAssertFalse(model.canBegin)
        model.address = "https://sync.example.com"
        XCTAssertFalse(model.canBegin, "an address without an account is not an account")
        model.accountId = "acct-1"
        XCTAssertTrue(model.canBegin)
    }

    /// The WebDAV form reads the account from the storage, so the folder is
    /// the whole of what has to be typed.
    func testAWebdavFolderIsEnoughOnItsOwn() {
        let model = PairingModel(store: store)
        model.target = .webdav
        model.address = "https://dav.example.com/typvia"
        XCTAssertTrue(model.canBegin)
    }

    /// A host with no gated key storage cannot hold a sync key, and then no
    /// address would have worked — so the screen says so before the reader
    /// fills anything in rather than after. Asserted as a rule rather than as
    /// a value, because the answer differs between a simulator and a device
    /// and a test that pins one of them is testing the machine.
    func testWithTheFieldsFilledTheOnlyThingLeftIsWhetherThisDeviceCanHoldAKey() async {
        let model = PairingModel(store: store)
        model.target = .webdav
        model.address = "https://dav.example.com/typvia"

        await model.load()

        // Unknown is not a no: a screen that refuses on the strength of a
        // question it never got an answer to is refusing for its own comfort.
        XCTAssertEqual(model.canBegin, model.canHoldAKey != false)
    }

    func testAServerThatCannotBeReachedLeavesTheScreenWhereItWas() async throws {
        let model = PairingModel(store: store)
        model.target = .server
        // Deliberately unroutable: nothing is listening, and nothing should be
        // reported as having joined.
        model.address = "http://127.0.0.1:9"
        model.accountId = "acct-1"

        let began = await model.begin()

        XCTAssertFalse(began)
        XCTAssertNil(model.step, "no code, because no session")
        XCTAssertNotNil(model.refusal, "and the reader is told")
    }

    /// Leaving takes the session with it. A code left running on a screen
    /// nobody is looking at is one somebody else could still answer.
    func testDroppingSaysNothingWasInstalledAndKeepsNoPasswords() {
        let model = PairingModel(store: store)
        model.masterPassword = "correct horse battery"
        model.password = "webdav-password"

        model.drop()

        XCTAssertEqual(model.step, .dropped)
        XCTAssertEqual(model.masterPassword, "")
        XCTAssertEqual(model.password, "")
        XCTAssertNil(model.offered)
        XCTAssertNil(model.reveal)
    }

    func testStartingOverGoesBackToTheQuestionRatherThanTheCode() {
        let model = PairingModel(store: store)
        model.drop()

        model.startOver()

        XCTAssertNil(model.step)
        XCTAssertNil(model.refusal)
    }

    /// The credential string's shape belongs to the transport. This asserts
    /// the screen is quoting it rather than writing its own.
    func testWebdavCredentialsArePackedByTheEngineAndAbsentWhenUnneeded() {
        XCTAssertNil(webdavCredentials(username: "", password: ""))
        XCTAssertEqual(
            webdavCredentials(username: "alice", password: "hunter2-FAKE"),
            "basic:alice:hunter2-FAKE"
        )
    }
}

/// Whether the reader has been introduced, and where that is remembered.
///
/// The marker is the desktop's — a file beside the database — rather than a
/// user default, which on iOS can outlive the app it belonged to. What is
/// under test is the round trip through the bridge and, more importantly, that
/// a fresh library says "not yet" instead of silently skipping the one thing
/// that explains the product.
@MainActor
final class FirstRunMarkerTests: XCTestCase {
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

    func testAFreshLibraryHasNotBeenIntroducedYet() async throws {
        let done = try await store.perform { $0.onboardingCompleted() }
        XCTAssertFalse(done)
    }

    func testBeingThroughItSurvivesTheAppBeingReopened() async throws {
        try await store.perform { try $0.markOnboardingComplete() }
        let marked = try await store.perform { $0.onboardingCompleted() }
        XCTAssertTrue(marked)

        // A second store over the same directory is what the next launch is.
        store = nil
        let reopened = try TypviaStore(dataDirectory: directory)
        let done = try await reopened.perform { $0.onboardingCompleted() }
        XCTAssertTrue(done, "the introduction does not come back for someone who has had it")
    }

    /// Saying it twice is not an error. The reader can finish the flow on one
    /// launch and skip it on another without the second one failing.
    func testSayingItTwiceIsStillDone() async throws {
        try await store.perform { try $0.markOnboardingComplete() }
        try await store.perform { try $0.markOnboardingComplete() }
        let done = try await store.perform { $0.onboardingCompleted() }
        XCTAssertTrue(done)
    }
}

/// The keyboard's bench, driven by a snapshot.
///
/// These assert behaviour rather than environment: the shared container may
/// or may not hold a snapshot on any given machine, and a test that assumes
/// one way or the other is testing the machine.
@MainActor
final class KeyboardModelTests: XCTestCase {
    func testWhatTheBenchReportsMatchesWhatItActuallyHas() {
        let model = KeyboardModel()
        model.load()
        XCTAssertEqual(
            model.reach.available, model.tiles.count,
            "the count in the search row is the list beside it, or it is a lie"
        )
        XCTAssertEqual(
            model.reach.available, model.reach.total,
            "the snapshot is all the bench has; a ratio would need a total it was never given"
        )
    }

    func testAnUnknownIdYieldsNothingRatherThanAnEmptyString() {
        // An empty string would be typed into the reader's document.
        let model = KeyboardModel()
        model.load()
        XCTAssertNil(model.body(for: "no-such-id"))
    }

    /// A sheet with nothing written on it is not a sheet. The snapshot hands
    /// over no title for a secret, so the panel draws the stand-in rather than
    /// an empty line — and the stand-in must not be mistakable for content.
    func testASecretSheetIsNamedEvenThoughTheSnapshotNamesNothing() {
        let entry = SnapshotEntry(
            id: "v9", title: "", snippetType: "", trigger: nil, folderId: nil,
            isFavorite: false, isRecent: false, isSensitive: true
        )

        let tile = KeyboardTile(entry: entry)

        XCTAssertEqual(tile.title, BodyPreview.shortMask)
        XCTAssertEqual(tile.sort, .secret, "and it is marked for what it is, not for what an empty kind falls back to")
        XCTAssertTrue(tile.isSecret)
    }

    func testAnOrdinarySheetKeepsItsOwnName() {
        let entry = SnapshotEntry(
            id: "n1", title: "Standup notes", snippetType: "text", trigger: nil,
            folderId: nil, isFavorite: false, isRecent: true, isSensitive: false
        )

        XCTAssertEqual(KeyboardTile(entry: entry).title, "Standup notes")
    }

    /// The shelf of "used recently" is built from these, and for a long time
    /// nothing on either mobile platform wrote one down: the shelf was empty
    /// forever, not because nobody used anything but because nobody recorded
    /// it. The rule now lives in the shared layer; this checks this host asks
    /// it, and that a use is one use.
    @MainActor
    func testDeliveringSomethingCountsExactlyOneUse() async throws {
        let directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
        let store = try TypviaStore(dataDirectory: directory)
        defer { try? FileManager.default.removeItem(at: directory) }
        let saved = try await store.perform {
            try $0.snippetCreate(
                draft: SnippetDraft(
                    title: "Rollback", body: "kubectl rollout undo", snippetType: "text",
                    description: nil, folderId: nil, trigger: nil, triggerMode: nil, language: nil
                )
            )
        }
        XCTAssertEqual(saved.usageCount, 0)
        XCTAssertNil(saved.lastUsedAt)

        let model = DetailModel(store: store, snippetId: saved.id)
        await model.load()
        await model.recordUse()

        let seen = try await store.perform { try $0.snippetGet(id: saved.id) }
        XCTAssertEqual(seen.usageCount, 1)
        XCTAssertNotNil(seen.lastUsedAt)
    }

    func testASecretsBodyNeverComesBackThroughThePanel() {
        let model = KeyboardModel()
        model.load()
        for tile in model.tiles where tile.isSecret {
            XCTAssertNil(model.body(for: tile.id), "a secret has no body to hand to a text field")
        }
    }

    func testTheBenchReturnsToWhereTheReaderWas() {
        let model = KeyboardModel()
        model.load()
        model.openSecret("id", title: "部署密钥")
        if case .secret = model.phase {} else { return XCTFail("expected the ink room") }
        model.backToBench()
        if case .browsing = model.phase { return }
        XCTFail("with no query typed, the bench goes back to browsing")
    }
}

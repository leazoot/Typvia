// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// The vault room's rules. The one that matters most is a rule about what is
/// *not* there: a shut vault publishes no count, no names, and no tally of
/// attempts.
@MainActor
final class VaultRoomTests: XCTestCase {
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

    // MARK: - What a shut vault says

    func testAVaultThatWasNeverMadeIsNotALockedDoor() async {
        let model = VaultModel(store: store)
        await model.load()
        XCTAssertEqual(model.phase, .absent, "asking a reader to unlock something that does not exist sends them looking for a key")
    }

    func testAShutVaultListsNothing() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: "correct horse battery") }
        _ = try await store.perform {
            try $0.vaultCreateSecret(
                draft: SnippetDraft(
                    title: "Deploy key", body: "AKIAFAKEEXAMPLE00000", snippetType: "sensitive",
                    description: nil, folderId: nil, trigger: nil, triggerMode: nil, language: nil
                )
            )
        }
        try await store.perform { _ = try $0.vaultLock() }

        let model = VaultModel(store: store)
        await model.load()
        XCTAssertEqual(model.phase, .shut)
        XCTAssertTrue(model.entries.isEmpty, "a shut vault holds no rows in memory either")
        XCTAssertNil(model.clock)
    }

    func testAWrongPasswordLeavesTheVaultShutAndSaysSoWithoutACount() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: "correct horse battery") }
        try await store.perform { _ = try $0.vaultLock() }

        let model = VaultModel(store: store)
        await model.open(password: "not the password")
        XCTAssertEqual(
            model.phase, .didNotOpen(.masterPassword),
            "the room has to know which door was tried; the refusal alone cannot say"
        )
        XCTAssertTrue(model.entries.isEmpty)
        // Whatever kind came back, what the reader is shown must not invent a
        // countdown: this vault has no attempt limit, and a screen that hints
        // at one is threatening somebody with a rule that does not exist.
        for language in UiLanguage.allCases {
            let shown = model.refusal?.sentence(Translator(language: language)) ?? ""
            XCTAssertFalse(shown.lowercased().contains("attempt"))
            XCTAssertFalse(shown.lowercased().contains("remaining"))
            XCTAssertFalse(shown.contains("次"))
        }
    }

    func testOpeningListsNamesAndTimesAndNoBodies() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: "correct horse battery") }
        _ = try await store.perform {
            try $0.vaultCreateSecret(
                draft: SnippetDraft(
                    title: "Deploy key", body: "AKIAFAKEEXAMPLE00000", snippetType: "sensitive",
                    description: nil, folderId: nil, trigger: nil, triggerMode: nil, language: nil
                )
            )
        }
        try await store.perform { _ = try $0.vaultLock() }

        let model = VaultModel(store: store)
        await model.open(password: "correct horse battery")
        XCTAssertEqual(model.phase, .open)
        XCTAssertEqual(model.entries.map(\.title), ["Deploy key"])
        XCTAssertNotNil(model.clock, "an open vault is always running out")
    }

    func testLeavingShutsItImmediately() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: "correct horse battery") }
        let model = VaultModel(store: store)
        await model.load()
        model.lockImmediately()
        XCTAssertEqual(model.phase, .shut)
        XCTAssertTrue(model.entries.isEmpty)
        XCTAssertNil(model.clock)
    }

    // MARK: - The idle line

    func testTheLineShrinksAcrossTheWholeWindow() {
        let full = RelockClock(idleTimeoutMs: 300_000, secondsLeft: 300)
        XCTAssertEqual(full.remaining, 1, accuracy: 0.0001)
        XCTAssertEqual(RelockClock(idleTimeoutMs: 300_000, secondsLeft: 150).remaining, 0.5, accuracy: 0.0001)
        XCTAssertEqual(RelockClock(idleTimeoutMs: 300_000, secondsLeft: -5).remaining, 0, accuracy: 0.0001)
        XCTAssertEqual(RelockClock(idleTimeoutMs: 0, secondsLeft: 10).remaining, 0, accuracy: 0.0001)
    }

    func testTheClockReadsAsMinutesAndSeconds() {
        XCTAssertEqual(RelockClock(idleTimeoutMs: 300_000, secondsLeft: 292).clockText, "4:52")
        XCTAssertEqual(RelockClock(idleTimeoutMs: 300_000, secondsLeft: 9).clockText, "0:09")
        XCTAssertEqual(RelockClock(idleTimeoutMs: 300_000, secondsLeft: -3).clockText, "0:00")
    }

    // MARK: - Order

    func testTheListIsOrderedByWhenEachWasLastTakenOut() {
        let sorted = VaultOrder.sort([
            VaultEntry(id: "a", title: "Older", lastUsedAt: 100),
            VaultEntry(id: "b", title: "Never", lastUsedAt: nil),
            VaultEntry(id: "c", title: "Newest", lastUsedAt: 900),
        ])
        XCTAssertEqual(sorted.map(\.title), ["Newest", "Older", "Never"])
    }

    func testTheOnesNeverTakenOutAreOrderedByNameRatherThanArbitrarily() {
        let sorted = VaultOrder.sort([
            VaultEntry(id: "a", title: "Beta", lastUsedAt: nil),
            VaultEntry(id: "b", title: "Alpha", lastUsedAt: nil),
        ])
        XCTAssertEqual(sorted.map(\.title), ["Alpha", "Beta"])
    }

    // MARK: - What the room says when a door did not open

    /// The one this was written for: a Face ID that was never given a copy of
    /// the key was never asked anything, so it cannot have failed to recognise
    /// anybody.
    func testAFaceIdWithNoKeyIsNotSaidToHaveFailedToRecogniseAnybody() {
        for language in UiLanguage.allCases {
            let tr = Translator(language: language)
            let said = VaultRoomCopy.headline(door: .faceId, refusal: .clash, tr)
                + VaultRoomCopy.detail(door: .faceId, refusal: .clash, tr)
            XCTAssertFalse(said.lowercased().contains("recognise"))
            XCTAssertFalse(said.contains("没认出"))
            XCTAssertFalse(said.lowercased().contains("try again"))
            XCTAssertFalse(said.contains("再试"))
        }
    }

    /// Trying a door whose answer cannot change is not offered as a way out.
    func testTheSameDoorIsOnlyOfferedAgainWhereItCouldAnswerDifferently() {
        XCTAssertFalse(VaultRoomCopy.retryCouldDiffer(door: .faceId, refusal: .clash))
        XCTAssertTrue(VaultRoomCopy.retryCouldDiffer(door: .faceId, refusal: .storage))
        // The field is already on the screen; that is the retry.
        XCTAssertFalse(VaultRoomCopy.retryCouldDiffer(door: .masterPassword, refusal: .notPermitted))
    }

    /// A wrong master password is not a sentence about Face ID, and the vault
    /// still being shut is said in every one of them.
    func testEachDoorAndKindGetsItsOwnSentence() {
        for language in UiLanguage.allCases {
            let tr = Translator(language: language)
            let cases: [(VaultDoor, Refusal)] = [
                (.faceId, .clash), (.faceId, .storage),
                (.masterPassword, .notPermitted), (.masterPassword, .storage),
            ]
            let headlines = cases.map { VaultRoomCopy.headline(door: $0.0, refusal: $0.1, tr) }
            XCTAssertEqual(Set(headlines).count, headlines.count, "one sentence stood in for another")
            for (door, refusal) in cases where door == .masterPassword {
                let said = VaultRoomCopy.headline(door: door, refusal: refusal, tr)
                XCTAssertFalse(said.contains("Face ID"))
                XCTAssertFalse(said.contains("面容"))
            }
            for headline in headlines {
                XCTAssertTrue(
                    headline.contains("still shut") || headline.contains("还是锁着的"),
                    "what is still true comes first, and it is that the vault is shut"
                )
            }
        }
    }

    /// A refusal that folds a wrong password together with too many tries must
    /// not be reported as either one. The detail may still say there is no
    /// attempt limit — that is the reassurance, not a diagnosis.
    func testTheWrongPasswordSentenceNamesNeitherOfTheTwoThingsItCouldBe() {
        for language in UiLanguage.allCases {
            let tr = Translator(language: language)
            let said = VaultRoomCopy.headline(door: .masterPassword, refusal: .notPermitted, tr)
            XCTAssertFalse(said.lowercased().contains("wrong"))
            XCTAssertFalse(said.lowercased().contains("too many"))
            XCTAssertFalse(said.contains("不对"))
            XCTAssertFalse(said.contains("太频繁"))
        }
    }
}

extension VaultEntry {
    /// Test-only construction. The production path builds these from snippets
    /// so that no call site can invent a vault row.
    init(id: String, title: String, lastUsedAt: Int64?) {
        self.init(
            snippet: Snippet(
                id: id, title: title, body: nil, snippetType: "sensitive",
                securityLevel: "sensitive", description: nil, folderId: nil, trigger: nil,
                triggerMode: nil, language: nil, isFavorite: false, isPinned: false,
                isEnabled: true, createdAt: 0, updatedAt: 0, lastUsedAt: lastUsedAt,
                usageCount: 0, version: 1, deletedAt: nil
            )
        )
    }
}

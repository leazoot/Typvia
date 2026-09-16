// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// The vault chapter: what it may state about a room it is standing outside
/// of, and what its two acts are allowed to leave behind.
@MainActor
final class VaultChapterTests: XCTestCase {
    private let en = Translator(language: .en)
    private let zh = Translator(language: .zh)
    private let password = "correct horse battery"

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

    // MARK: - The window, said in whole minutes

    /// Rounding down to "0 minutes" would read as "it does not re-lock", which
    /// is the opposite of what a forty-second window means.
    func testAWindowShorterThanAMinuteIsNeverSaidAsZero() {
        let brief = VaultFacts(isMade: true, idleTimeoutMs: 40_000)
        XCTAssertEqual(VaultChapterCopy.idleWindow(brief, en), "under a minute")
        XCTAssertEqual(VaultChapterCopy.contentsValue(brief, en), "shuts in under a min")
        XCTAssertFalse(VaultChapterCopy.contentsValue(brief, zh).contains("0"))
    }

    func testTheWindowAgreesWithItselfInBothColumns() {
        let five = VaultFacts(isMade: true, idleTimeoutMs: 300_000)
        XCTAssertEqual(VaultChapterCopy.idleWindow(five, en), "5 minutes")
        XCTAssertEqual(VaultChapterCopy.contentsValue(five, en), "shuts after 5 min")
        XCTAssertEqual(VaultChapterCopy.contentsValue(five, zh), "5 分钟回锁")
        // English's plural is decided in one place, so a one-minute window does
        // not read as "1 minutes".
        XCTAssertEqual(
            VaultChapterCopy.idleWindow(VaultFacts(isMade: true, idleTimeoutMs: 60_000), en),
            "1 minute"
        )
    }

    /// A device with no vault has no window to announce: the number would be a
    /// setting for something that does not exist.
    func testAVaultThatWasNeverMadeStatesNoWindowAtAll() {
        let none = VaultFacts(isMade: false, idleTimeoutMs: 300_000)
        XCTAssertNil(VaultChapterCopy.idleWindow(none, en))
        XCTAssertEqual(VaultChapterCopy.contentsValue(none, en), "not made yet")
        XCTAssertNotEqual(VaultChapterCopy.headline(none, en), VaultChapterCopy.headline(none, zh))
        XCTAssertNil(VaultChapterCopy.idleWindow(VaultFacts(isMade: true, idleTimeoutMs: 0), en))
    }

    // MARK: - What a refusal says

    /// The core answers a wrong master password and too many attempts in a row
    /// with the same kind. Naming one of them would be a guess printed as a
    /// fact, so the sentence says what did not happen instead.
    func testTheRefusalForAFailedOpenNamesNeitherOfTheTwoThingsItCouldBe() {
        let sentence = VaultChapterCopy.refusal(.notPermitted, en)
        XCTAssertFalse(sentence.lowercased().contains("attempt"))
        XCTAssertFalse(sentence.lowercased().contains("wrong"))
        XCTAssertTrue(sentence.contains("Nothing changed"))
        XCTAssertFalse(VaultChapterCopy.refusal(.notPermitted, zh).contains("次"))
    }

    /// The shared sentence for this kind is "something here is already taken",
    /// which on this page would be about nothing at all.
    func testNoRefusalOnThisPageBorrowsASentenceWrittenForSomewhereElse() {
        for refusal in [Refusal.notPermitted, .clash, .storage] {
            for tr in [en, zh] {
                XCTAssertNotEqual(
                    VaultChapterCopy.refusal(refusal, tr),
                    refusal.sentence(tr),
                    "\(refusal)"
                )
            }
        }
        XCTAssertFalse(VaultChapterCopy.refusal(.clash, en).contains("already taken"))
        XCTAssertFalse(VaultChapterCopy.refusal(.clash, zh).contains("已经被占用"))
    }

    /// A failed act leaves the vault exactly as it was, and the page says so:
    /// a page that reports only the failure leaves a reader guessing.
    func testAFailedActSaysWhatIsStillTrue() {
        XCTAssertTrue(VaultChapterCopy.refusal(.storage, en).contains("as it was"))
        XCTAssertTrue(VaultChapterCopy.refusal(.storage, zh).contains("原来的样子"))
    }

    // MARK: - The two acts

    /// The red line of doing this from settings: enrolling needs the key in
    /// hand, so the vault is opened here — and it must be shut again before
    /// the reader has a chance to walk away from an open one.
    func testKeepingACopyNeverLeavesAnOpenVaultBehindIt() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: password) }
        try await store.perform { _ = try $0.vaultLock() }
        let model = SettingsModel(store: store)
        await model.load()

        await model.useFaceId(password: password, en)

        let status = try await store.perform { try $0.vaultStatus() }
        XCTAssertFalse(status.unlocked, "settings opened the vault to enrol and left it open")
        // Whether the key store took the copy depends on the machine this runs
        // on; that it reported one thing or the other does not.
        XCTAssertTrue(
            (model.vaultSaid == nil) != (model.vaultRefusal == nil),
            "the act says what it did, and says one thing"
        )
    }

    func testAMasterPasswordThatDoesNotOpenItChangesNothing() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: password) }
        try await store.perform { _ = try $0.vaultLock() }
        let model = SettingsModel(store: store)
        await model.load()

        await model.useFaceId(password: "not the password", en)

        XCTAssertEqual(model.vaultRefusal, .notPermitted)
        XCTAssertNil(model.vaultSaid)
        let status = try await store.perform { try $0.vaultStatus() }
        XCTAssertFalse(status.unlocked)
        XCTAssertTrue(status.initialized, "a failed enrolment is not a reset")
    }

    /// Red line: what the reader typed is key input, and it does not come back
    /// out in anything this page prints.
    func testNothingThePageSaysCarriesWhatWasTyped() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: password) }
        try await store.perform { _ = try $0.vaultLock() }
        let model = SettingsModel(store: store)

        for typed in [password, "not the password"] {
            await model.useFaceId(password: typed, en)
            let printed = [model.vaultSaid, model.vaultRefusal.map { VaultChapterCopy.refusal($0, en) }]
            for sentence in printed.compactMap({ $0 }) {
                XCTAssertFalse(sentence.contains(typed), sentence)
            }
        }
    }

    /// Removing the copy asks for nothing: a key copy taken away is not a way
    /// into anything, and the master password is untouched either way.
    func testRemovingTheCopyNeedsNoPasswordAndReportsWhatHappened() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: password) }
        try await store.perform { _ = try $0.vaultLock() }
        let model = SettingsModel(store: store)
        await model.load()

        await model.stopUsingFaceId(en)

        XCTAssertTrue((model.vaultSaid == nil) != (model.vaultRefusal == nil))
        let status = try await store.perform { try $0.vaultStatus() }
        XCTAssertTrue(status.initialized, "removing a Face ID copy is not removing the vault")
        XCTAssertFalse(status.unlocked)
    }

    /// A report belongs to the act that produced it. Leaving the page ends it,
    /// so re-opening the chapter does not congratulate anyone on yesterday.
    func testLeavingThePageEndsTheReport() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: password) }
        let model = SettingsModel(store: store)
        await model.stopUsingFaceId(en)

        model.clearVaultReport()

        XCTAssertNil(model.vaultSaid)
        XCTAssertNil(model.vaultRefusal)
    }

    /// The chapter reads the vault from the core rather than keeping a figure
    /// of its own — the contents page and the page itself are then the same
    /// number by construction.
    func testTheChapterReadsTheVaultFromTheCore() async throws {
        let model = SettingsModel(store: store)
        await model.load()
        XCTAssertFalse(model.vault.isMade)

        _ = try await store.perform { try $0.vaultInitialize(password: password) }
        await model.load()

        XCTAssertTrue(model.vault.isMade)
        XCTAssertGreaterThan(model.vault.idleTimeoutMs, 0, "an open vault is always running out")
    }
}

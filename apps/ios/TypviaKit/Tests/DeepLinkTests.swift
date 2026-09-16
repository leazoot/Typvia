// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// The one channel another app can aim at this one.
///
/// The cases mirror the ones the retired shell was held to: a link is either
/// one of a short closed list or it is nothing, and a link that arrives during
/// the first run waits rather than being dropped or jumped to.
final class DeepLinkTests: XCTestCase {
    func testASearchLinkCarriesItsQuery() {
        XCTAssertEqual(parse("typvia://search?q=deploy"), .search("deploy"))
        XCTAssertEqual(parse("typvia://search"), .search(""))
    }

    func testASnippetLinkCarriesItsId() {
        XCTAssertEqual(parse("typvia://snippet/abc-123"), .snippet("abc-123"))
    }

    func testASnippetLinkWithNoIdLandsHomeRatherThanNowhere() {
        XCTAssertEqual(parse("typvia://snippet"), .home)
        XCTAssertEqual(parse("typvia://snippet/"), .home)
    }

    func testAnotherAppsSchemeIsNotOurs() {
        // Not "home" — nothing at all. Reacting to somebody else's link is how
        // an app becomes something other apps can steer.
        XCTAssertNil(parse("https://example.com"))
        XCTAssertNil(parse("othertool://snippet/abc"))
    }

    func testAHostThisVersionDoesNotKnowLandsHome() {
        XCTAssertEqual(parse("typvia://something-new/xyz"), .home)
        XCTAssertEqual(parse("typvia://"), .home)
    }

    func testTheSchemeIsMatchedWithoutRegardToCase() {
        XCTAssertEqual(parse("TYPVIA://Search?q=x"), .search("x"))
    }

    // MARK: - Arriving during the first run

    @MainActor
    func testALinkThatArrivesDuringTheFirstRunWaits() {
        let router = DeepLinkRouter(isOnboarding: true)
        router.receive(.snippet("abc"))
        XCTAssertNil(router.pending, "jumping there would strand a reader in a product they have not met")

        router.onboardingFinished()
        XCTAssertEqual(router.pending, .snippet("abc"), "and dropping it would lose what they tapped")
    }

    @MainActor
    func testTwoTapsBeforeTheAppIsReadyMeanTheSecondThing() {
        let router = DeepLinkRouter(isOnboarding: true)
        router.receive(.snippet("first"))
        router.receive(.search("second"))
        router.onboardingFinished()
        XCTAssertEqual(router.pending, .search("second"))
    }

    @MainActor
    func testAfterTheFirstRunALinkActsAtOnce() {
        let router = DeepLinkRouter(isOnboarding: false)
        router.receive(.search("deploy"))
        XCTAssertEqual(router.pending, .search("deploy"))
        router.clear()
        XCTAssertNil(router.pending)
    }

    @MainActor
    func testFinishingTheFirstRunWithNothingWaitingChangesNothing() {
        let router = DeepLinkRouter(isOnboarding: true)
        router.onboardingFinished()
        XCTAssertNil(router.pending)
    }

    private func parse(_ string: String) -> DeepLink? {
        guard let url = URL(string: string) else { return nil }
        return DeepLink.parse(url)
    }
}

/// A link that points at something that is gone.
@MainActor
final class MissingSnippetTests: XCTestCase {
    func testOpeningASnippetThatIsGoneReportsItRatherThanShowingAnEmptyPage() async throws {
        let directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try TypviaStore(dataDirectory: directory)

        let model = DetailModel(store: store, snippetId: "no-such-id")
        await model.load()
        XCTAssertNil(model.snippet)
        XCTAssertEqual(model.refusal, .missing, "a blank page leaves the reader guessing")
    }
}

/// Waking up to sync. What can be checked without a device is the policy:
/// when a wake-up is worth asking for, and that the identifier matches the one
/// the app declares.
final class BackgroundSyncTests: XCTestCase {
    func testTheIdentifierIsTheOneTheAppDeclares() {
        // The system refuses a registration whose identifier is not in the
        // Info.plist, and it refuses it at launch.
        XCTAssertEqual(BackgroundSync.taskId, "dev.typvia.mobile.refresh")
    }

    func testAWakeUpIsOnlyWorthAskingForWhenThereIsSomethingToSend() {
        XCTAssertTrue(BackgroundSync.shouldSchedule(isConfigured: true, isEnabled: true))
        XCTAssertFalse(BackgroundSync.shouldSchedule(isConfigured: true, isEnabled: false))
        XCTAssertFalse(
            BackgroundSync.shouldSchedule(isConfigured: false, isEnabled: true),
            "sync that was never set up has nothing to send, and waking for it spends a budget the system gives sparingly"
        )
        XCTAssertFalse(BackgroundSync.shouldSchedule(isConfigured: false, isEnabled: false))
    }

    func testTheIntervalIsARequestNotAPromise() {
        // Fifteen minutes is what iOS treats as the floor for app refresh; the
        // system still decides when, or whether, it runs at all.
        XCTAssertEqual(BackgroundSync.interval, 15 * 60, accuracy: 0.001)
    }
}

/// What a background round leaves behind.
///
/// Nobody is looking at the app while this runs, which is exactly why the
/// document the extensions read has to be rewritten by the round itself: if it
/// is not, it stays as it was until the reader next opens the app, and the
/// keyboard goes on offering the library as it stood before the sync.
@MainActor
final class BackgroundRoundSnapshotTests: XCTestCase {
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

    /// The publish a round performs is the same one the app performs — there
    /// is one writer and one document, not a background copy of either.
    func testARoundRepublishesTheSameDocumentTheAppDoes() async throws {
        let shared = directory.appendingPathComponent("shared")
        try FileManager.default.createDirectory(at: shared, withIntermediateDirectories: true)
        _ = try await store.perform {
            try $0.snippetCreate(
                draft: SnippetDraft(
                    title: "Tail logs", body: "docker logs -f app", snippetType: "command",
                    description: nil, folderId: nil, trigger: ";dlog",
                    triggerMode: "delimiter", language: nil
                )
            )
        }

        let written = await SnapshotPublish.write(store: store, to: shared)

        XCTAssertTrue(written)
        let document = shared.appendingPathComponent("snapshot.json")
        XCTAssertTrue(FileManager.default.fileExists(atPath: document.path))
        let json = try String(contentsOf: document, encoding: .utf8)
        XCTAssertEqual(try widgetEntries(json: json, limit: 5).map(\.title), ["Tail logs"])
    }
}

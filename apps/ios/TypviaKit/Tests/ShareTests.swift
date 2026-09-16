// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// The share sheet and the widgets. The rule with the most at stake is the
/// widget one: a widget sits on a screen other people can see.
final class ShareTests: XCTestCase {
    // MARK: - What the sheet guesses

    func testTheTitleIsTheFirstLineTrimmedToARowsWorth() {
        XCTAssertEqual(
            SharedDraft.title(from: "  部署前请先确认 CI 全绿。\n第二行不要。 "),
            "部署前请先确认 CI 全绿。"
        )
        XCTAssertEqual(SharedDraft.title(from: ""), "")
    }

    func testALongFirstLineIsCutToSomethingThatFitsARow() {
        let long = String(repeating: "x", count: 200)
        XCTAssertEqual(SharedDraft.title(from: long).count, SharedDraft.titleLimit)
    }

    func testTheStripOffersFourKindsBeforeAnySwiping() {
        XCTAssertEqual(SharedDraft.offered.count, 4)
        XCTAssertEqual(SharedDraft.offered.first, .text)
    }

    // MARK: - A taken trigger gets a digit, not an interruption

    func testAFreeTriggerIsLeftAlone() {
        XCTAssertEqual(TriggerSuggestion.next(after: ";ci", taken: []), ";ci")
    }

    func testATakenTriggerGetsTheNextFreeSpelling() {
        XCTAssertEqual(TriggerSuggestion.next(after: ";ci", taken: [";ci"]), ";ci2")
        XCTAssertEqual(TriggerSuggestion.next(after: ";ci", taken: [";ci", ";ci2"]), ";ci3")
        XCTAssertEqual(
            TriggerSuggestion.next(after: ";ci", taken: [";ci", ";ci2", ";ci3", ";ci4"]),
            ";ci5"
        )
    }

    // MARK: - What a widget is allowed to show

    func testAWidgetRowCarriesNoBodyAtAll() {
        let row = WidgetRow(entry: entry(title: "部署脚本", type: "code", trigger: ";deploy"))
        XCTAssertEqual(row.title, "部署脚本")
        XCTAssertEqual(row.trigger, ";deploy")
        XCTAssertEqual(row.sort, .code)
    }

    func testASecretsTriggerIsWithheldAlongWithEverythingElse() {
        // A trigger is a thing you can type at a keyboard to get the value
        // out; printing it on a home screen hands over half the door.
        let row = WidgetRow(
            entry: entry(title: "部署密钥", type: "sensitive", trigger: ";key", sensitive: true)
        )
        XCTAssertNil(row.trigger)
        XCTAssertTrue(row.isSecret)
        XCTAssertEqual(row.sort, .secret)
    }

    func testTheLockScreenShowsNoRowsAtAll() {
        XCTAssertFalse(WidgetSize.lock.showsRows)
        XCTAssertEqual(WidgetSize.lock.rowLimit, 0)
    }

    func testTheSmallAndMediumSizesShowWhatTheFramesDraw() {
        XCTAssertEqual(WidgetSize.small.rowLimit, 1)
        XCTAssertEqual(WidgetSize.medium.rowLimit, 3)
    }

    private func entry(
        title: String, type: String, trigger: String? = nil, sensitive: Bool = false
    ) -> SnapshotEntry {
        SnapshotEntry(
            id: UUID().uuidString, title: title, snippetType: type, trigger: trigger,
            folderId: nil, isFavorite: false, isRecent: true, isSensitive: sensitive
        )
    }
}

/// The App Group channels. Both are one-way, and the inbox has one property
/// worth pinning: a second share must not overwrite the first.
final class SharedContainerTests: XCTestCase {
    override func tearDown() {
        Inbox.clear()
        super.tearDown()
    }

    func testTheTwoChannelsAreDistinctFilesInTheGroupContainer() {
        XCTAssertEqual(SharedContainer.appGroup, "group.dev.typvia.mobile")
        XCTAssertNotEqual(SharedContainer.snapshotURL, SharedContainer.inboxURL)
        XCTAssertEqual(SharedContainer.snapshotURL?.lastPathComponent, "snapshot.json")
        XCTAssertEqual(SharedContainer.inboxURL?.lastPathComponent, "inbox.json")
    }

    func testASecondShareDoesNotOverwriteTheFirst() {
        Inbox.clear()
        XCTAssertTrue(Inbox.append(item(title: "First")))
        XCTAssertTrue(Inbox.append(item(title: "Second")))
        // The failure this guards against is the one nobody notices until
        // something they shared is simply gone.
        XCTAssertEqual(Inbox.read().map(\.title), ["First", "Second"])
    }

    func testClearingIsWhatTheAppDoesAfterFiling() {
        Inbox.clear()
        XCTAssertTrue(Inbox.append(item(title: "Only")))
        Inbox.clear()
        XCTAssertTrue(Inbox.read().isEmpty)
    }

    func testAnInboxItemSurvivesAJsonRoundTrip() {
        Inbox.clear()
        let original = item(title: "部署前提醒")
        XCTAssertTrue(Inbox.append(original))
        XCTAssertEqual(Inbox.read(), [original])
    }

    private func item(title: String) -> InboxItem {
        InboxItem(
            title: title, body: "确认 CI 全绿", trigger: ";ci",
            snippetType: "text", sharedAt: 1_755_800_000_000
        )
    }
}

/// Filing what the share sheet left. The two properties that matter: nothing
/// shared is lost, and the inbox is only cleared for what actually landed.
@MainActor
final class InboxIntakeTests: XCTestCase {
    private var directory: URL!
    private var store: TypviaStore!

    override func setUpWithError() throws {
        directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
        store = try TypviaStore(dataDirectory: directory)
        Inbox.clear()
    }

    override func tearDownWithError() throws {
        Inbox.clear()
        store = nil
        try? FileManager.default.removeItem(at: directory)
    }

    func testAnEmptyInboxIsNotWork() async {
        let outcomes = await InboxIntake.run(store: store)
        XCTAssertTrue(outcomes.isEmpty)
    }

    func testWhatWasSharedEndsUpInTheLibraryAndTheInboxIsCleared() async throws {
        Inbox.append(item(title: "部署前提醒", trigger: ";ci"))
        let outcomes = await InboxIntake.run(store: store)
        XCTAssertEqual(outcomes.count, 1)
        guard case .filed = outcomes[0] else { return XCTFail("expected it to be filed") }
        XCTAssertTrue(Inbox.read().isEmpty)

        let rows = try await store.perform {
            try $0.snippetListPage(view: "all", folderId: nil, snippetType: nil, limit: 10, offset: 0)
        }
        XCTAssertEqual(rows.map(\.title), ["部署前提醒"])
        XCTAssertEqual(rows.first?.trigger, ";ci")
    }

    func testATakenTriggerCostsTheTriggerRatherThanTheText() async throws {
        _ = try await store.perform {
            try $0.snippetCreate(
                draft: SnippetDraft(
                    title: "First", body: "x", snippetType: "text", description: nil,
                    folderId: nil, trigger: ";ci", triggerMode: "delimiter", language: nil
                )
            )
        }
        Inbox.append(item(title: "Shared", trigger: ";ci"))

        let outcomes = await InboxIntake.run(store: store)
        guard case .filedWithoutTrigger = outcomes.first else {
            return XCTFail("a name collision must not cost the reader their text")
        }
        let rows = try await store.perform {
            try $0.snippetListPage(view: "all", folderId: nil, snippetType: nil, limit: 10, offset: 0)
        }
        XCTAssertEqual(rows.count, 2)
        XCTAssertEqual(rows.filter { $0.title == "Shared" }.first?.trigger, nil)
        XCTAssertTrue(Inbox.read().isEmpty)
    }

    func testTwoSharesBothLand() async {
        Inbox.append(item(title: "One", trigger: nil))
        Inbox.append(item(title: "Two", trigger: nil))
        let outcomes = await InboxIntake.run(store: store)
        XCTAssertEqual(outcomes.count, 2)
        XCTAssertTrue(Inbox.read().isEmpty)
    }

    private func item(title: String, trigger: String?) -> InboxItem {
        InboxItem(
            title: title, body: "确认 CI 全绿", trigger: trigger,
            snippetType: "text", sharedAt: 1_755_800_000_000
        )
    }
}

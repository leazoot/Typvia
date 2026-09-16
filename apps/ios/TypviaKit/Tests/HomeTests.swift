// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// The home screen's rules, as values.
///
/// What is under test is what the screen decides: which state it is in, what a
/// row is allowed to show, how far the sheet travels. The pixels are a walk-
/// through; these are the parts that can be wrong without anybody noticing.
final class HomeTests: XCTestCase {
    // MARK: - Which of the six states

    func testAQueryPutsTheScreenInSearchWhateverElseIsTrue() {
        XCTAssertEqual(HomePhase.resolve(isLoaded: false, total: 0, query: "de"), .searching)
        XCTAssertEqual(HomePhase.resolve(isLoaded: true, total: 132, query: "de"), .searching)
    }

    func testWhitespaceIsNotAQuery() {
        XCTAssertEqual(HomePhase.resolve(isLoaded: true, total: 132, query: "   "), .resting)
    }

    func testNothingIsEmptyUntilItHasBeenRead() {
        XCTAssertEqual(HomePhase.resolve(isLoaded: false, total: 0, query: ""), .loading)
        XCTAssertEqual(HomePhase.resolve(isLoaded: true, total: 0, query: ""), .empty)
    }

    func testAStockedLibraryRests() {
        XCTAssertEqual(HomePhase.resolve(isLoaded: true, total: 1, query: ""), .resting)
    }

    // MARK: - A secret is never previewed

    func testASensitiveRowWithholdsItsBodyEvenIfOneArrives() {
        // The bridge already returns nil for a sensitive body. This asserts the
        // screen would still not print one if that ever changed.
        let row = SearchResult(
            snippet: snippet(
                title: "Deploy key",
                body: "AKIAFAKEEXAMPLE00000",
                type: "sensitive",
                security: "sensitive",
                trigger: ";key"
            ),
            query: ";ke"
        )
        XCTAssertEqual(row.preview, .withheld)
        XCTAssertEqual(row.tail, .needsVerification)
        XCTAssertFalse(BodyPreview.mask.contains("AKIA"))
    }

    func testAnUnknownSecurityLevelIsTreatedAsSensitive() {
        let row = SearchResult(
            snippet: snippet(
                title: "From a newer version",
                body: "AKIAFAKEEXAMPLE00000",
                type: "text",
                security: "vault-only"
            ),
            query: ""
        )
        XCTAssertEqual(row.preview, .withheld)
    }

    func testTheMaskDoesNotPublishTheLengthOfWhatItHides() {
        let short = SearchResult(
            snippet: snippet(title: "A", body: "x", type: "sensitive", security: "sensitive"),
            query: ""
        )
        let long = SearchResult(
            snippet: snippet(
                title: "B",
                body: String(repeating: "x", count: 400),
                type: "sensitive",
                security: "sensitive"
            ),
            query: ""
        )
        XCTAssertEqual(short.preview, long.preview)
    }

    func testANormalRowShowsItsBodyAndItsTrigger() {
        let row = SearchResult(
            snippet: snippet(
                title: "Deploy",
                body: "./deploy.sh --env=prod",
                type: "code",
                security: "normal",
                trigger: ";deploy"
            ),
            query: ";de"
        )
        XCTAssertEqual(row.preview, .text("./deploy.sh --env=prod"))
        XCTAssertEqual(row.sort, .code)
        XCTAssertEqual(row.tail, .trigger(TriggerMatch(trigger: ";deploy", query: ";de")))
    }

    // MARK: - The matched head of a trigger

    func testTheTypedHeadIsSplitOffSoItCanBeUnderlined() {
        let match = TriggerMatch(trigger: ";deploy", query: ";de")
        XCTAssertEqual(match.matched, ";de")
        XCTAssertEqual(match.rest, "ploy")
    }

    func testMatchingIgnoresCaseButKeepsTheTriggersOwn() {
        let match = TriggerMatch(trigger: ";Deploy", query: ";de")
        XCTAssertEqual(match.matched, ";De")
        XCTAssertEqual(match.rest, "ploy")
    }

    func testAQueryThatHitTheTitleUnderlinesNothing() {
        // The core matched on something other than the trigger, so there is no
        // head to mark: the whole trigger stays plain.
        let match = TriggerMatch(trigger: ";deploy", query: "script")
        XCTAssertEqual(match.matched, "")
        XCTAssertEqual(match.rest, ";deploy")
    }

    func testAMidTriggerMatchIsNotUnderlinedFromTheMiddle() {
        let match = TriggerMatch(trigger: ";deploy", query: "ploy")
        XCTAssertEqual(match.matched, "")
        XCTAssertEqual(match.rest, ";deploy")
    }

    // MARK: - The shelf

    func testTheTriggerListKeepsOnlyLinesThatHaveATriggerToShow() {
        let rows = [
            snippet(title: "Signature", body: "…", type: "text", trigger: ";sig"),
            snippet(title: "No trigger", body: "…", type: "text"),
            snippet(title: "Cluster login", body: "…", type: "command", trigger: ";k8s"),
        ]
        let shelf = HomeShelf.assemble(
            total: 3,
            recent: rows,
            mostUsed: rows,
            vaultLocked: false,
            queued: 0,
            lastSyncAt: nil
        )
        XCTAssertEqual(shelf.triggers.map(\.title), ["Signature", "Cluster login"])
        XCTAssertEqual(shelf.tiles.count, 3, "the cards keep the entry with no trigger")
    }

    /// The two sections answer two questions and are filled from two lists.
    /// "Used recently" is the recent order; the trigger lines are headed with
    /// what gets typed most, and come from the usage order — printing the
    /// recent list under that heading is a heading over the wrong rows.
    func testTheCardsAndTheTriggerLinesComeFromDifferentLists() {
        let shelf = HomeShelf.assemble(
            total: 2,
            recent: [snippet(title: "Just edited", body: "…", type: "text", trigger: ";new")],
            mostUsed: [snippet(title: "Typed daily", body: "…", type: "text", trigger: ";old")],
            vaultLocked: false,
            queued: 0,
            lastSyncAt: nil
        )
        XCTAssertEqual(shelf.tiles.map(\.title), ["Just edited"])
        XCTAssertEqual(shelf.triggers.map(\.title), ["Typed daily"])
    }

    func testALockedVaultClosesTheTriggerListAndCarriesNoTitleOfItsOwn() {
        let shelf = HomeShelf.assemble(
            total: 1,
            recent: [snippet(title: "Signature", body: "…", type: "text", trigger: ";sig")],
            mostUsed: [snippet(title: "Signature", body: "…", type: "text", trigger: ";sig")],
            vaultLocked: true,
            queued: 0,
            lastSyncAt: nil
        )
        XCTAssertEqual(shelf.triggers.count, 2)
        XCTAssertEqual(shelf.triggers.last?.kind, .lockedVault)
        XCTAssertEqual(shelf.triggers.last?.sort, .secret)
        XCTAssertTrue(
            shelf.triggers.last?.title.isEmpty == true,
            "the vault line's sentence belongs to the view, in the reader's language"
        )
    }

    func testAnUnlockedVaultAddsNoLine() {
        let shelf = HomeShelf.assemble(
            total: 0, recent: [], vaultLocked: false, queued: 0, lastSyncAt: nil
        )
        XCTAssertTrue(shelf.triggers.isEmpty)
    }

    func testTheTriggerListIsCappedButTheCardsAreNot() {
        let recent = (0..<10).map {
            snippet(title: "Snippet \($0)", body: "…", type: "text", trigger: ";t\($0)")
        }
        let shelf = HomeShelf.assemble(
            total: 10,
            recent: recent,
            mostUsed: recent,
            vaultLocked: false,
            queued: 0,
            lastSyncAt: nil
        )
        XCTAssertEqual(shelf.triggers.count, HomeShelf.triggerLimit)
        XCTAssertEqual(shelf.tiles.count, 10)
    }

    func testTheCardsKeepTheOrderTheCoreGaveThem() {
        let recent = [
            snippet(title: "First", body: "…", type: "text"),
            snippet(title: "Second", body: "…", type: "text"),
        ]
        let shelf = HomeShelf.assemble(
            total: 2, recent: recent, vaultLocked: false, queued: 0, lastSyncAt: nil
        )
        XCTAssertEqual(shelf.tiles.map(\.title), ["First", "Second"])
    }

    // MARK: - A stopped sync round

    func testAStoppedRoundCarriesItsReasonOntoTheNotice() {
        let shelf = HomeShelf.assemble(
            total: 2,
            recent: [],
            vaultLocked: false,
            queued: 4,
            lastSyncAt: nil,
            syncFailure: .auth
        )
        XCTAssertEqual(shelf.syncFailure, .auth)
    }

    /// A queue with nothing behind it is the ordinary case — changes saved
    /// while sync had not run yet. There is no cause to report, and the shelf
    /// must not manufacture one.
    func testAQueueWithNoFailureBehindItReportsNoCause() {
        let shelf = HomeShelf.assemble(
            total: 2, recent: [], vaultLocked: false, queued: 4, lastSyncAt: nil
        )
        XCTAssertNil(shelf.syncFailure)
    }

    /// Only the failures a second attempt could actually clear offer one.
    /// The rest send the reader where the decision lives, and the one the
    /// server itself asked to wait out offers nothing at all.
    func testOnlyFailuresARetryCouldClearOfferARetry() {
        XCTAssertEqual(SyncFailureKind.unreachable.remedy, .tryAgain)
        XCTAssertEqual(SyncFailureKind.serverRefused.remedy, .tryAgain)
        XCTAssertEqual(SyncFailureKind.serverUnexpected.remedy, .tryAgain)
        XCTAssertEqual(SyncFailureKind.thisDevice.remedy, .tryAgain)

        XCTAssertEqual(SyncFailureKind.auth.remedy, .openSettings)
        XCTAssertEqual(SyncFailureKind.trust.remedy, .openSettings)
        XCTAssertEqual(SyncFailureKind.serverAddress.remedy, .openSettings)
        XCTAssertEqual(SyncFailureKind.protocolVersion.remedy, .openSettings)
        XCTAssertEqual(SyncFailureKind.notSetUp.remedy, .openSettings)

        XCTAssertEqual(SyncFailureKind.busy.remedy, .waitItOut)
    }

    /// Every category has a sentence in both languages, and neither of them
    /// is the other. A missing half is how a single-language screen turns
    /// into a mixed one.
    func testEveryCauseIsWrittenInBothLanguages() {
        let kinds: [SyncFailureKind] = [
            .unreachable, .serverAddress, .auth, .protocolVersion, .busy,
            .serverRefused, .serverUnexpected, .trust, .notSetUp, .thisDevice,
        ]
        let en = Translator(language: .en)
        let zh = Translator(language: .zh)
        for kind in kinds {
            XCTAssertFalse(kind.cause(en).isEmpty)
            XCTAssertFalse(kind.cause(zh).isEmpty)
            XCTAssertNotEqual(kind.cause(en), kind.cause(zh))
        }
    }

    // MARK: - Sorts

    func testEveryCoreTypeThisDesignHasASortForMapsToIt() {
        let pairs: [(String, TypeSort)] = [
            ("text", .text), ("code", .code), ("command", .command), ("prompt", .prompt),
            ("template", .template), ("sensitive", .secret), ("ai_action", .aiAction),
            ("link", .link),
        ]
        for (coreType, sort) in pairs {
            XCTAssertEqual(TypeSort(coreType: coreType), sort)
        }
    }

    func testATypeWithNoSortDrawsNoMarkRatherThanBorrowingOne() {
        XCTAssertNil(TypeSort(coreType: "temporary"))
        XCTAssertNil(TypeSort(coreType: "something_a_newer_version_added"))
    }

    // MARK: - Pulling the sheet

    func testThePaperTravelsLessFarThanTheFinger() {
        XCTAssertEqual(PullToReveal.travel(forDrag: 100), 55, accuracy: 0.001)
    }

    func testThereIsNoBenchAboveThePaper() {
        XCTAssertEqual(PullToReveal.travel(forDrag: -120), 0)
    }

    func testTheThresholdIsMeasuredOnWhatTheReaderCanSee() {
        // 64pt of travel, which the damping puts at ~116pt of finger.
        XCTAssertFalse(PullToReveal.isPastThreshold(PullToReveal.travel(forDrag: 100)))
        XCTAssertTrue(PullToReveal.isPastThreshold(PullToReveal.travel(forDrag: 120)))
    }

    func testOnlyAShelfAtItsTopHandsTheDragToThePull() {
        XCTAssertTrue(PullToReveal.yieldsToPull(scrollOffset: 0))
        // Rounding around the top edge is still the top edge.
        XCTAssertTrue(PullToReveal.yieldsToPull(scrollOffset: -0.4))
        // Scrolled down, the same drag means "back up" and belongs to the shelf.
        XCTAssertFalse(PullToReveal.yieldsToPull(scrollOffset: -40))
    }

    // MARK: - One language at a time

    func testSimplifiedChineseReadersGetChinese() {
        XCTAssertEqual(UiLanguage.resolve(preferred: ["zh-Hans-CN", "en-US"]), .zh)
        XCTAssertEqual(UiLanguage.resolve(preferred: ["zh"]), .zh)
    }

    func testEverybodyElseGetsEnglishRatherThanAMixture() {
        XCTAssertEqual(UiLanguage.resolve(preferred: ["en-GB", "zh-Hans"]), .en)
        XCTAssertEqual(UiLanguage.resolve(preferred: ["ja-JP"]), .en)
        XCTAssertEqual(UiLanguage.resolve(preferred: []), .en)
    }

    func testTraditionalChineseGetsEnglishBecauseThereIsNoCopyForIt() {
        XCTAssertEqual(UiLanguage.resolve(preferred: ["zh-Hant-TW"]), .en)
    }

    func testATranslatorRendersOneHalfAndOnlyOne() {
        XCTAssertEqual(Translator(language: .en)("Search", "搜索"), "Search")
        XCTAssertEqual(Translator(language: .zh)("Search", "搜索"), "搜索")
    }

    // MARK: - Fixtures

    private func snippet(
        title: String,
        body: String?,
        type: String,
        security: String = "normal",
        trigger: String? = nil
    ) -> Snippet {
        Snippet(
            id: UUID().uuidString,
            title: title,
            body: body,
            snippetType: type,
            securityLevel: security,
            description: nil,
            folderId: nil,
            trigger: trigger,
            triggerMode: trigger.map { _ in "delimiter" },
            language: nil,
            isFavorite: false,
            isPinned: false,
            isEnabled: true,
            createdAt: 0,
            updatedAt: 0,
            lastUsedAt: nil,
            usageCount: 0,
            version: 1,
            deletedAt: nil
        )
    }
}

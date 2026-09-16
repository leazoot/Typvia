// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// The specimen book's rules: eight chapters that share one skeleton and
/// differ in exactly three ways, one of which is that a chapter of secrets
/// tells you nothing at all.
final class LibraryTests: XCTestCase {
    // MARK: - Eight chapters, three differences

    func testEveryChapterPreviewsOneLineOrTwoAndNeverAThirdOption() {
        for sort in TypeSort.allCases {
            let style = ChapterStyle.of(sort)
            XCTAssertTrue(style.previewLines == 1 || style.previewLines == 2, sort.code)
        }
    }

    func testOnlyTheCommandChapterShowsItsBodyWhole() {
        let whole = TypeSort.allCases.filter { !ChapterStyle.of($0).truncates }
        XCTAssertEqual(whole, [.command], "cutting a command's tail cuts off which server it runs on")
    }

    func testOnlyTheTextChapterIsFiledUnderFolders() {
        let grouped = TypeSort.allCases.filter { ChapterStyle.of($0).groupsByFolder }
        XCTAssertEqual(grouped, [.text])
    }

    // MARK: - Folders

    func testAGroupedChapterKeepsTheFoldersOrderAndPutsLooseRowsLast() {
        let chapter = Chapter(
            sort: .text,
            phase: .listed,
            count: 4,
            rows: [
                row("Loose one"),
                row("Signature", folder: "work"),
                row("Address", folder: "home"),
                row("Reply", folder: "work"),
            ],
            folders: [FolderRef(id: "work", name: "Work"), FolderRef(id: "home", name: "Home")]
        )
        let groups = chapter.groups
        XCTAssertEqual(groups.map(\.title), ["Work", "Home", nil])
        XCTAssertEqual(groups[0].rows.map(\.title), ["Signature", "Reply"])
        XCTAssertEqual(groups[2].rows.map(\.title), ["Loose one"])
    }

    func testAnEmptiedFolderGetsNoHeading() {
        let chapter = Chapter(
            sort: .text,
            phase: .listed,
            count: 1,
            rows: [row("Address", folder: "home")],
            folders: [FolderRef(id: "work", name: "Work"), FolderRef(id: "home", name: "Home")]
        )
        XCTAssertEqual(chapter.groups.map(\.title), ["Home"])
    }

    func testARowInAFolderThisPageDidNotFetchStaysVisible() {
        let chapter = Chapter(
            sort: .text,
            phase: .listed,
            count: 1,
            rows: [row("Nested", folder: "deep")],
            folders: [FolderRef(id: "work", name: "Work")]
        )
        XCTAssertEqual(chapter.groups.map(\.title), [nil])
        XCTAssertEqual(chapter.groups.first?.rows.map(\.title), ["Nested"])
    }

    func testAnUngroupedChapterIsOneRunWithNoHeading() {
        let chapter = Chapter(
            sort: .command, phase: .listed, count: 1, rows: [row("Rollback")]
        )
        XCTAssertEqual(chapter.groups.count, 1)
        XCTAssertNil(chapter.groups.first?.title)
    }

    func testOnlyCodeIsSetOnAGrid() {
        let gridded = TypeSort.allCases.filter { ChapterStyle.of($0).material == .grid }
        XCTAssertEqual(gridded, [.code])
    }

    func testOnlyProseGetsASecondPreviewLine() {
        let twoLines = TypeSort.allCases.filter { ChapterStyle.of($0).previewLines == 2 }
        XCTAssertEqual(twoLines, [.prompt])
    }

    func testOnlyTheAiChapterChangesTheAccentAndOnlySecretsLeaveTheLibraryRoom() {
        XCTAssertEqual(ChapterStyle.of(.aiAction).accent, .ai)
        XCTAssertEqual(ChapterStyle.of(.secret).accent, .vault)
        for sort in [TypeSort.text, .code, .command, .template, .link] {
            XCTAssertEqual(ChapterStyle.of(sort).accent, .library, "\(sort.code) stays ochre")
        }
    }

    func testTheSortBarPrintsEightChaptersInTheDeliverysOrder() {
        XCTAssertEqual(
            Chapter.order.map(\.code),
            ["TX", "CD", "CM", "PR", "TP", "SC", "AI", "LK"]
        )
    }

    // MARK: - The shut chapter

    func testAShutChapterPublishesNoCount() {
        let locked = Chapter(sort: .secret, phase: .locked, count: 0, rows: [])
        XCTAssertFalse(locked.showsCount)
    }

    func testEveryOtherStateShowsItsCount() {
        for phase in [ChapterPhase.listed, .empty, .loading, .unavailable] {
            let chapter = Chapter(sort: .text, phase: phase, count: 41, rows: [])
            XCTAssertTrue(chapter.showsCount)
        }
    }

    func testASecretRowIsWithheldWhicheverChapterItIsReadIn() {
        for sort in TypeSort.allCases {
            let row = LibraryRow(
                snippet: snippet(
                    title: "Deploy key",
                    body: "AKIAFAKEEXAMPLE00000",
                    type: "sensitive",
                    security: "sensitive"
                ),
                sort: sort
            )
            XCTAssertEqual(row.preview, .withheld, "\(sort.code) must not preview a secret")
        }
    }

    // MARK: - Links

    func testTheLinkChapterShowsWhereALinkGoesNotTheWholeAddress() {
        let row = LibraryRow(
            snippet: snippet(
                title: "Status page",
                body: "https://status.example.com/incidents/2026-08?tab=history",
                type: "link"
            ),
            sort: .link
        )
        XCTAssertEqual(row.preview, .text("status.example.com"))
    }

    func testSomethingThatIsNotAnAddressIsShownAsItIs() {
        let row = LibraryRow(
            snippet: snippet(title: "Note to self", body: "not a url", type: "link"),
            sort: .link
        )
        XCTAssertEqual(row.preview, .text("not a url"))
    }

    func testOtherChaptersShowTheBodyWhole() {
        let row = LibraryRow(
            snippet: snippet(
                title: "Deploy",
                body: "https://example.com/webhook",
                type: "command"
            ),
            sort: .command
        )
        XCTAssertEqual(row.preview, .text("https://example.com/webhook"))
    }

    // MARK: - Turning pages

    func testAShortDragDoesNotTurnThePage() {
        XCTAssertEqual(ChapterPaging.step(forDrag: -60, width: 390), 0)
    }

    func testDraggingLeftGoesForwardAndRightGoesBack() {
        XCTAssertEqual(ChapterPaging.step(forDrag: -160, width: 390), 1)
        XCTAssertEqual(ChapterPaging.step(forDrag: 160, width: 390), -1)
    }

    func testTheLeftEdgeBelongsToTheSystemsBackGesture() {
        XCTAssertFalse(ChapterPaging.canBegin(atX: 12))
        XCTAssertTrue(ChapterPaging.canBegin(atX: 40))
    }

    func testTheBookDoesNotWrapAround() {
        let count = Chapter.order.count
        XCTAssertEqual(ChapterPaging.chapter(after: 0, step: -1, count: count), 0)
        XCTAssertEqual(ChapterPaging.chapter(after: count - 1, step: 1, count: count), count - 1)
        XCTAssertEqual(ChapterPaging.chapter(after: 3, step: 1, count: count), 4)
    }

    // MARK: - Template blanks

    func testATemplateRowCarriesItsBlankCount() {
        let row = LibraryRow(
            snippet: snippet(title: "Standup", body: "Yesterday {{a}} today {{b}}", type: "template"),
            sort: .template,
            slots: 2
        )
        XCTAssertEqual(row.slots, 2)
    }

    func testEveryOtherChapterLeavesTheBlankCountAlone() {
        let row = LibraryRow(
            snippet: snippet(title: "Note", body: "no blanks here", type: "text"),
            sort: .text
        )
        XCTAssertNil(row.slots)
    }

    func testTheNumberOfBlanksInASecretIsNotPublished() {
        let row = LibraryRow(
            snippet: snippet(
                title: "Key",
                body: "AKIAFAKEEXAMPLE00000",
                type: "sensitive",
                security: "sensitive"
            ),
            sort: .template,
            slots: 3
        )
        XCTAssertNil(row.slots, "a secret's shape is part of the secret")
        XCTAssertEqual(row.preview, .withheld)
    }

    // MARK: - Reading a chapter a page at a time

    func testAChapterKnowsWhenThereIsMoreBehindIt() {
        let full = Chapter(sort: .text, phase: .listed, count: 120, rows: [], hasMore: true)
        XCTAssertTrue(full.hasMore)
        let done = Chapter(sort: .text, phase: .listed, count: 2, rows: [], hasMore: false)
        XCTAssertFalse(done.hasMore)
    }

    func testTheNextPageLandsUnderTheRowsAlreadySet() {
        let first = Chapter(
            sort: .text,
            phase: .listed,
            count: 4,
            rows: [row("One"), row("Two")],
            hasMore: true
        )
        let second = first.appending([row("Three"), row("Four")], hasMore: false)
        XCTAssertEqual(second.rows.map(\.title), ["One", "Two", "Three", "Four"])
        XCTAssertFalse(second.hasMore)
        XCTAssertEqual(second.count, 4, "the total is the chapter's, not the page's")
        XCTAssertEqual(second.sort, first.sort)
        XCTAssertEqual(second.phase, first.phase)
    }

    // MARK: - The mark settles last

    func testTheMarkRisesInTheClosingStretchOfATurn() {
        let beat = Beat.transition.duration
        let delay = ChapterPaging.markLiftDelay(in: beat)
        let rise = ChapterPaging.markLiftDuration(in: beat)
        XCTAssertEqual(delay + rise, beat, accuracy: 0.0001, "the rise ends with the turn")
        XCTAssertEqual(rise / beat, 0.4, accuracy: 0.0001)
        XCTAssertGreaterThan(ChapterPaging.markLift, 0)
    }

    // MARK: - The core's vocabulary

    func testEveryChaptersCoreTypeRoundTripsBackToItsSort() {
        for sort in TypeSort.allCases {
            XCTAssertEqual(TypeSort(coreType: sort.coreType), sort)
        }
    }

    // MARK: - Fixtures

    private func row(_ title: String, folder: String? = nil) -> LibraryRow {
        LibraryRow(
            snippet: snippet(title: title, body: "…", type: "text", folderId: folder),
            sort: .text
        )
    }

    private func snippet(
        title: String,
        body: String?,
        type: String,
        security: String = "normal",
        trigger: String? = nil,
        folderId: String? = nil
    ) -> Snippet {
        Snippet(
            id: UUID().uuidString,
            title: title,
            body: body,
            snippetType: type,
            securityLevel: security,
            description: nil,
            folderId: folderId,
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

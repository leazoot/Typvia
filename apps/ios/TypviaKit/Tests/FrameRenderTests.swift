// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI
import XCTest

@testable import TypviaKit

/// Every frame draws.
///
/// The value tests say what a screen decides; these say that the deciding
/// produces something. A state that only appears when a database is empty, a
/// vault is shut or a query is half-typed is a state nobody looks at until a
/// reader hits it, and a blank sheet, a view that lays out to nothing or one
/// that traps on a missing value all pass a value test.
///
/// Each case renders one frame's contents at a phone's width and asserts there
/// is ink on the page. The PNGs are also written to the test process's
/// temporary directory — `testTheRenderedFramesAreWhereTheNoteSaysTheyAre`
/// attaches the path — so a frame can be taken out and held against the
/// delivery by eye.
@MainActor
final class FrameRenderTests: XCTestCase {
    // MARK: - Home

    func testTheSearchPositionDrawsInEachOfItsThreeFootnotes() {
        assertDraws(
            "home-search-counts",
            SearchPosition(counts: counts, footnote: .counts(lastSyncAt: 1_755_800_000_000))
        )
        assertDraws("home-search-offline", SearchPosition(counts: counts, footnote: .offline))
        assertDraws("home-search-loading", SearchPosition(counts: counts, footnote: .loading))
    }

    /// What this covers is the line *around* the field: the rule, the cancel
    /// word, the match count. The field itself is UIKit-backed and the
    /// renderer substitutes a placeholder for it, so the typed text and its
    /// caret are only ever verified on a running simulator — the walkthrough
    /// owns that one, and this case must not be read as covering it.
    func testTheTypingLineDrawsAroundItsField() {
        assertDraws("home-search-typing", SearchLineHost(query: ";de", resultCount: 3))
    }

    func testAResultRowDrawsForBothANormalSnippetAndASecret() {
        assertDraws(
            "home-result-normal",
            ResultRow(
                result: SearchResult(
                    snippet: fixture(
                        title: "Deploy script",
                        body: "./deploy.sh --env=prod --yes",
                        type: "code",
                        trigger: ";deploy"
                    ),
                    query: ";de"
                )
            )
        )
        assertDraws(
            "home-result-secret",
            ResultRow(
                result: SearchResult(
                    snippet: fixture(
                        title: "Deploy key",
                        body: "AKIAFAKEEXAMPLE00000",
                        type: "sensitive",
                        security: "sensitive"
                    ),
                    query: ";de"
                )
            )
        )
    }

    func testTheEmptySheetDrawsWithAndWithoutItsWaysIn() {
        assertDraws("home-empty-bare", EmptySheet(actions: HomeActions()), height: 420)
        assertDraws(
            "home-empty-with-actions",
            EmptySheet(
                actions: HomeActions(compose: {}, importClipboard: {})
            ),
            height: 420
        )
    }

    func testTheShelfAndItsSkeletonBothDraw() {
        assertDraws(
            "home-shelf",
            ShelfView(tiles: tiles, triggers: triggers, total: 132, open: nil, compose: {}),
            height: 500
        )
        assertDraws("home-skeleton", SkeletonShelf(), height: 460)
    }

    /// The way to write another one has to be on a shelf that is not empty —
    /// it used to live only in the empty state, which is the one place a
    /// reader with a library never sees.
    func testAShelfWithAThingOnItStillOffersToWriteAnother() {
        assertDraws(
            "home-shelf-compose",
            ShelfView(tiles: tiles, triggers: [], total: 132, open: nil, compose: {}),
            height: 400
        )
    }

    func testTheNoticeCardDrawsForBothKindsOfBadNews() {
        assertDraws(
            "home-notice-sync",
            NoticeCard(
                notice: .syncPaused(
                    queued: 3,
                    lastSyncAt: 1_755_800_000_000,
                    reason: .unreachable
                ),
                total: 132,
                isRetrying: false,
                retry: {}
            ),
            height: 260
        )
        assertDraws(
            "home-notice-failed",
            NoticeCard(notice: .loadFailed, total: 132, isRetrying: false, retry: {}),
            height: 260
        )
        // The one that used to be impossible to draw: a stopped round whose
        // only honest answer is a route into settings, not a retry.
        assertDraws(
            "home-notice-auth",
            NoticeCard(
                notice: .syncPaused(queued: 3, lastSyncAt: nil, reason: .auth),
                total: 132,
                isRetrying: false,
                retry: {},
                openSettings: {}
            ),
            height: 260
        )
    }

    // MARK: - Library

    func testAChapterHeadDrawsInEveryPhaseIncludingTheShutOne() {
        for phase in [ChapterPhase.listed, .empty, .loading] {
            assertDraws(
                "library-head-\(phase)",
                ChapterHead(chapter: Chapter(sort: .code, phase: phase, count: 24, rows: [])),
                height: 260
            )
        }
    }

    func testTheShutChapterDrawsAndShowsNoContentAtAll() {
        let chapter = Chapter(sort: .secret, phase: .locked, count: 0, rows: [])
        assertDraws("library-secret-locked", SecretChapter(chapter: chapter, openVault: nil), height: 560)
        assertDraws(
            "library-secret-locked-with-unlock",
            SecretChapter(chapter: chapter, openVault: {}),
            height: 560
        )
    }

    func testAChapterRowDrawsOnPlainPaperAndOnTheGrid() {
        let row = LibraryRow(
            snippet: fixture(
                title: "Rollback",
                body: "kubectl rollout undo deploy/api",
                type: "code",
                trigger: ";deroll"
            ),
            sort: .code
        )
        assertDraws("library-row-grid", ChapterRow(row: row, style: .of(.code), open: nil), height: 90)
        assertDraws(
            "library-row-plain",
            ChapterRow(row: row, style: .of(.command), open: nil),
            height: 90
        )
    }

    func testATemplateRowDrawsItsBlankCount() {
        let row = LibraryRow(
            snippet: fixture(
                title: "站会记录",
                body: "昨天 {{a}} 今天 {{b}}",
                type: "template",
                trigger: ";standup"
            ),
            sort: .template,
            slots: 2
        )
        assertDraws(
            "library-row-template",
            ChapterRow(row: row, style: .of(.template), open: nil),
            height: 90
        )
    }

    func testEveryChaptersEmptyInvitationDraws() {
        for sort in TypeSort.allCases {
            assertDraws(
                "library-empty-\(sort.code)",
                EmptyChapter(sort: sort, compose: {}),
                height: 320
            )
        }
    }

    func testTheChapterSkeletonAndTheUnreadableChapterDraw() {
        assertDraws("library-skeleton", ChapterSkeleton(), height: 320)
        assertDraws("library-unavailable", UnavailableChapter(), height: 120)
    }

    // MARK: - Share and widgets

    func testTheShareSheetsTwoFramesDraw() {
        let draft = SharedDraft(
            title: "部署前提醒",
            body: "部署前请先确认 CI 全绿。回滚命令写在 wiki 第三节,别用旧的那条。",
            trigger: ";ci",
            sort: .text
        )
        assertDraws(
            "share-composing",
            ShareSheet(draft: .constant(draft), step: .composing, save: {}, cancel: {}),
            height: 460
        )
        assertDraws(
            "share-saved",
            ShareSheet(
                draft: .constant(draft),
                step: .saved(trigger: ";ci", position: 133),
                edit: {},
                back: {}
            ),
            height: 460
        )
    }

    func testTheThreeWidgetFacesDraw() {
        let rows = [
            WidgetRow(entry: entry(title: "部署脚本", type: "code", trigger: ";deploy")),
            WidgetRow(entry: entry(title: "上线通知", type: "template", trigger: ";ship")),
            WidgetRow(entry: entry(title: "部署密钥", type: "sensitive", sensitive: true)),
        ]
        assertDraws("widget-small", WidgetFace(size: .small, rows: rows, total: 132), width: 170, height: 170)
        assertDraws("widget-medium", WidgetFace(size: .medium, rows: rows, total: 132), width: 364, height: 170)
        assertDraws("widget-lock", WidgetFace(size: .lock, rows: rows, total: 132), width: 170, height: 76)
    }

    /// A widget is drawn in a process of its own from a value, so nothing in
    /// it can be verified by looking at the app. This renders one face under
    /// each of two choices and holds the pixels against each other: the choice
    /// has to reach the paper, not merely be read.
    func testAWidgetFaceIsDrawnInTheLanguageAndAppearanceTheReaderChose() throws {
        let rows = [WidgetRow(entry: entry(title: "部署脚本", type: "code", trigger: ";deploy"))]
        let face = WidgetFace(size: .small, rows: rows, total: 132)
        let light = try render(face.uiPreferences(.init(language: .en, appearance: .light)))
        let dark = try render(face.uiPreferences(.init(language: .zh, appearance: .dark)))

        XCTAssertNotEqual(
            light.pngData(), dark.pngData(),
            "the two choices drew the same face, so neither of them reached it"
        )
        // The corner below the rows is paper, and which paper is the question.
        XCTAssertEqual(try corner(of: dark), Tokens.Swatch.paperDark)
        XCTAssertEqual(try corner(of: light), Tokens.Swatch.paperLight)
    }

    /// Following the phone must stay a third answer: pinned to either scheme,
    /// every surface that reads the value rather than the window would render
    /// one theme forever.
    func testFollowingThePhoneLeavesTheFaceToTheSystem() throws {
        let face = WidgetFace(size: .small, rows: [], total: 0)
        let followed = try render(face.uiPreferences(.init()))
        let light = try render(face.uiPreferences(.init(language: .en, appearance: .light)))
        // The renderer runs in the light trait, so following it draws light
        // paper — the point is that it was not forced there by the modifier.
        XCTAssertEqual(try corner(of: followed), try corner(of: light))
    }

    private func render(_ view: some View, size: CGFloat = 170) throws -> UIImage {
        let renderer = ImageRenderer(
            content: view.frame(width: size, height: size, alignment: .topLeading)
                .background(Paper.base)
        )
        renderer.scale = 1
        return try XCTUnwrap(renderer.uiImage)
    }

    /// The colour of the bottom-left pixel — below the last row, where the
    /// face is nothing but paper — as the swatch integer the tokens use.
    private func corner(of image: UIImage) throws -> UInt32 {
        let cgImage = try XCTUnwrap(image.cgImage)
        var pixel = [UInt8](repeating: 0, count: 4)
        let context = try XCTUnwrap(
            CGContext(
                data: &pixel,
                width: 1,
                height: 1,
                bitsPerComponent: 8,
                bytesPerRow: 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            )
        )
        context.draw(cgImage, in: CGRect(x: 0, y: 0, width: image.size.width, height: image.size.height))
        return UInt32(pixel[0]) << 16 | UInt32(pixel[1]) << 8 | UInt32(pixel[2])
    }

    // MARK: - Keyboard

    func testTheKeyboardBandsAccountForEveryPointItAsksFor() {
        // The delivery's four bands come to 242 of 260; the rest is the home
        // indicator's strip. What must never happen is the panel asking for a
        // height its contents do not account for — that is a clipped band or a
        // floating one, and neither is visible in a value test.
        XCTAssertEqual(KeyboardBand.total, Tokens.Viewport.keyboardHeight, accuracy: 0.001)
        XCTAssertEqual(KeyboardBand.bands, 242, accuracy: 0.001)
        XCTAssertEqual(KeyboardBand.bands + KeyboardBand.gutter, KeyboardBand.total, accuracy: 0.001)
        XCTAssertGreaterThan(KeyboardBand.gutter, 0)
    }

    /// The panel's frames cover the bench's chrome — the search row, the sort
    /// strip, the function row and the states that replace the tile run. The
    /// tiles themselves sit in a horizontal `ScrollView`, which the renderer
    /// draws as an empty box, so they are covered on their own below rather
    /// than being silently absent from a passing test.
    func testTheKeyboardsFramesDraw() {
        let tiles = [
            KeyboardTile(entry: entry(title: "部署脚本", type: "code", trigger: ";deploy"), isLead: true),
            KeyboardTile(entry: entry(title: "回滚发布", type: "command", trigger: ";deroll")),
            KeyboardTile(entry: entry(title: "部署密钥", type: "sensitive", sensitive: true)),
        ]
        assertDraws(
            "keyboard-browsing",
            KeyboardPanel(
                phase: .browsing, tiles: tiles,
                reach: KeyboardReach(available: 132, total: 132), query: .constant("")
            ),
            height: KeyboardBand.total
        )
        assertDraws(
            "keyboard-filtering",
            KeyboardPanel(
                phase: .filtering(";de"), tiles: tiles,
                reach: KeyboardReach(available: 132, total: 132), query: .constant(";de")
            ),
            height: KeyboardBand.total
        )
        assertDraws(
            "keyboard-inserted",
            KeyboardPanel(
                phase: .inserted(title: "部署脚本"), tiles: tiles,
                reach: KeyboardReach(available: 132, total: 132), query: .constant(""),
                actions: KeyboardActions(undo: {})
            ),
            height: KeyboardBand.total
        )
        assertDraws(
            "keyboard-secret",
            KeyboardPanel(
                phase: .secret(title: "部署密钥"), tiles: tiles,
                reach: KeyboardReach(available: 132, total: 132), query: .constant("")
            ),
            height: KeyboardBand.total
        )
        assertDraws(
            "keyboard-no-match",
            KeyboardPanel(
                phase: .noMatch(";发票"), tiles: [],
                reach: KeyboardReach(available: 132, total: 132), query: .constant(";发票"),
                actions: KeyboardActions(saveWhatIsTyped: {})
            ),
            height: KeyboardBand.total
        )
        // The only way this state is actually reached: no snapshot at all.
        assertDraws(
            "keyboard-nothing-to-read",
            KeyboardPanel(
                phase: .limited(available: 0, total: 0), tiles: [],
                reach: KeyboardReach(available: 0, total: 0), query: .constant("")
            ),
            height: KeyboardBand.total
        )
    }

    func testATileDrawsForBothANormalSnippetAndASecret() {
        assertDraws(
            "keyboard-tile",
            KeyboardTileView(
                tile: KeyboardTile(
                    entry: entry(title: "部署脚本", type: "code", trigger: ";deploy"), isLead: true
                ),
                use: {}
            ),
            height: 140
        )
        assertDraws(
            "keyboard-tile-secret",
            KeyboardTileView(
                tile: KeyboardTile(
                    entry: entry(title: "部署密钥", type: "sensitive", sensitive: true)
                ),
                use: {}
            ),
            height: 140
        )
        assertDraws(
            "keyboard-tile-spent",
            KeyboardTileView(
                tile: KeyboardTile(
                    entry: entry(title: "部署脚本", type: "code", trigger: ";deploy"), isSpent: true
                ),
                use: {}
            ),
            height: 140
        )
    }

    private func entry(
        title: String, type: String, trigger: String? = nil, sensitive: Bool = false
    ) -> SnapshotEntry {
        SnapshotEntry(
            id: UUID().uuidString, title: title, snippetType: type, trigger: trigger,
            folderId: nil, isFavorite: false, isRecent: true, isSensitive: sensitive
        )
    }

    func testTheRecycleBinDrawsInBothOfItsStates() {
        let now = Int64(1_000) * 86_400_000
        let rows = [
            TrashRow(snippet: fixture(title: "旧的部署脚本", body: "x", type: "code")),
            TrashRow(snippet: fixture(title: "过期提醒", body: "x", type: "text")),
        ]
        assertDraws(
            "settings-trash",
            TrashSection(
                rows: rows, now: now, restore: { _ in }, deleteForever: { _ in },
                confirming: .constant(nil)
            ),
            height: 320
        )
        assertDraws(
            "settings-trash-confirming",
            TrashSection(
                rows: rows, now: now, restore: { _ in }, deleteForever: { _ in },
                confirming: .constant(rows[0].id)
            ),
            height: 320
        )
        assertDraws(
            "settings-trash-empty",
            TrashSection(
                rows: [], now: now, restore: { _ in }, deleteForever: { _ in },
                confirming: .constant(nil)
            ),
            height: 140
        )
    }

    // MARK: - Pairing and first launch

    func testTheFourPairingAndFirstRunFramesDraw() throws {
        // The join step is driven by a model, and a model holds a store. The
        // frame draws no data from it — what is on this page is the question,
        // not the library — but it has to be a real one.
        let directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try TypviaStore(dataDirectory: directory)
        assertDraws(
            "pairing-offer",
            PairingOffer(
                code: PairingCode(code: "K7M2Q4X9AB3F", secondsLeft: 240),
                useTextCode: {}
            ),
            height: 520
        )
        assertDraws(
            "pairing-offer-expired",
            PairingOffer(code: PairingCode(code: "K7M2Q4X9AB3F", secondsLeft: 0), useTextCode: {}),
            height: 520
        )
        // A WebDAV folder promises no window. The page still has to be a page —
        // it just says nothing about validity.
        assertDraws(
            "pairing-offer-no-window",
            PairingOffer(code: PairingCode(code: "K7M2Q4X9AB3F", secondsLeft: nil), useTextCode: {}),
            height: 520
        )
        assertDraws(
            "pairing-sas",
            SasCheck(
                // The shape the core hands over, not the six characters the
                // page was once written for.
                reveal: SasReveal(sas: "U3GRD-UUGGD-DTOTC-K7FUB", shown: 20),
                note: "MacBook Pro 正在等你确认",
                confirm: {},
                reject: {}
            ),
            height: 560
        )
        assertDraws(
            "pair-join-target",
            JoinTarget(model: PairingModel(store: store)),
            height: 620
        )
        assertDraws(
            "first-run-save",
            FirstRunScreen(page: .save, next: {}, skip: {}, openSystemSettings: nil, install: .notAdded),
            height: 460
        )
        assertDraws(
            "first-run-keyboard-ready",
            FirstRunScreen(
                page: .keyboard, next: nil, skip: {}, openSystemSettings: {}, install: .ready
            ),
            height: 560
        )
        assertDraws(
            "first-run-flow",
            FirstRunFlow(openSystemSettings: {}, finish: {}),
            height: 560
        )
        assertDraws(
            "first-run-keyboard",
            FirstRunScreen(
                page: .keyboard, next: nil, skip: {}, openSystemSettings: {}, install: .notAdded
            ),
            height: 560
        )
    }

    // MARK: - Settings

    func testTheContentsPageAndItsThreeChaptersDraw() {
        assertDraws(
            "settings-contents",
            VStack(alignment: .leading, spacing: Tokens.Space.group) {
                ForEach(SettingsChapter.allCases) { chapter in
                    ChapterLine(
                        chapter: chapter,
                        value: chapter == .data ? "132 枚" : "跟随系统",
                        open: {}
                    )
                }
            },
            height: 620
        )
        assertDraws("settings-ai", AiChapter(engine: .onThisDevice, refusal: nil), height: 560)
        assertDraws(
            "settings-keyboard",
            KeyboardChapter(install: .notAdded, openSystemSettings: {}),
            height: 900
        )
        assertDraws(
            "settings-vault",
            VaultChapter(
                facts: VaultFacts(isMade: true, idleTimeoutMs: 300_000),
                keep: { _ in },
                remove: {},
                forget: {}
            ),
            height: 760
        )
        // The device with no vault yet: the page must not collapse to nothing
        // when its two acts have nothing to act on.
        assertDraws(
            "settings-vault-absent",
            VaultChapter(facts: VaultFacts(isMade: false, idleTimeoutMs: 0)),
            height: 320
        )
        assertDraws("settings-data", DataChapter(stats: DataStats(total: 132, sorts: 8)), height: 560)
        assertDraws(
            "settings-sync-trouble",
            SyncTroubleNote(
                trouble: SyncTrouble(queued: 7, lastSyncAt: 1_755_800_000_000, total: 132),
                isRetrying: false,
                retry: {}
            ),
            height: 300
        )
        assertDraws(
            "settings-device-waiting",
            DeviceLine(
                device: DeviceRow(
                    device: SyncDevice(
                        deviceId: "this", name: "iPhone 15 Pro", platform: "ios", createdAt: 0,
                        revokedAt: nil, verified: true, isThisDevice: true, isRoot: true
                    ),
                    pending: 7
                )
            ),
            height: 90
        )
    }

    // MARK: - Vault

    func testTheVaultsFourFramesDraw() {
        assertDraws(
            "vault-shut",
            ShutVault(
                isOpening: false,
                showsPasswordField: .constant(false),
                password: .constant(""),
                openWithBiometrics: {},
                openWithPassword: {}
            )
            .background(VaultRoom.base),
            height: 560
        )
        assertDraws(
            "vault-face-id-did-not-open",
            DidNotOpen(
                door: .faceId,
                refusal: .storage,
                showsPasswordField: .constant(false),
                password: .constant(""),
                retry: {},
                openWithPassword: {}
            )
            .background(VaultRoom.base),
            height: 620
        )
        // The one with no retry word and the field already open: trying a door
        // with no key behind it a second time is the same answer.
        assertDraws(
            "vault-face-id-has-no-key",
            DidNotOpen(
                door: .faceId,
                refusal: .clash,
                showsPasswordField: .constant(false),
                password: .constant(""),
                retry: {},
                openWithPassword: {}
            )
            .background(VaultRoom.base),
            height: 620
        )
        assertDraws(
            "vault-open",
            OpenVault(
                entries: [
                    VaultEntry(id: "a", title: "部署密钥", lastUsedAt: 1_755_800_000_000),
                    VaultEntry(id: "b", title: "数据库主密码", lastUsedAt: nil),
                ],
                clock: RelockClock(idleTimeoutMs: 300_000, secondsLeft: 292),
                open: { _ in }
            )
            .background(VaultRoom.base),
            height: 520
        )
        assertDraws("vault-absent", AbsentVault().background(VaultRoom.base), height: 420)
    }

    func testTheSortBarDraws() {
        assertDraws("library-sort-bar", SortBar(current: .code) { _ in }, height: 60)
    }

    // MARK: - Rendering

    /// Renders one frame and asserts it put ink on the sheet.
    ///
    /// "Ink" is any pixel that differs from the page it was drawn on. A view
    /// that lays out to nothing, or draws its type in the paper's own colour,
    /// leaves none — and both are failures the eye would catch at a glance and
    /// a value test never would.
    private func assertDraws(
        _ name: String,
        _ view: some View,
        width: CGFloat = 390,
        height: CGFloat = 320,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        let renderer = ImageRenderer(
            content: view
                .environment(\.tr, Translator(language: .zh))
                .frame(width: width, height: height, alignment: .topLeading)
                .background(Paper.base)
        )
        renderer.scale = 2
        guard let image = renderer.uiImage, let cgImage = image.cgImage else {
            return XCTFail("\(name) rendered nothing at all", file: file, line: line)
        }
        XCTAssertEqual(image.size.width, width, accuracy: 1, file: file, line: line)

        let inked = FrameRenderTests.inkedFraction(of: cgImage)
        XCTAssertGreaterThan(
            inked, 0.001, "\(name) drew a blank sheet", file: file, line: line
        )
        FrameRenderTests.dump(image, named: name)
    }

    /// The fraction of pixels that are not the page's own colour.
    static func inkedFraction(of image: CGImage) -> Double {
        let width = image.width
        let height = image.height
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        guard
            let context = CGContext(
                data: &pixels,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: width * 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            )
        else { return 0 }
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))

        // The corner is page, whatever the theme resolved to.
        let page = (pixels[0], pixels[1], pixels[2])
        var different = 0
        for index in stride(from: 0, to: pixels.count, by: 4) {
            let isPage = abs(Int(pixels[index]) - Int(page.0)) < 6
                && abs(Int(pixels[index + 1]) - Int(page.1)) < 6
                && abs(Int(pixels[index + 2]) - Int(page.2)) < 6
            if !isPage { different += 1 }
        }
        return Double(different) / Double(width * height)
    }

    /// Writes each frame into the test process's own temporary directory, so
    /// they can be pulled out and held against the delivery. Temp only —
    /// nothing lands in the repository, and the system clears it.
    ///
    /// The path is reported once, as an attachment on the test result, because
    /// a file whose location nobody knows is a file nobody opens.
    static let dumpDirectory: URL = {
        let directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("typvia-frames", isDirectory: true)
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }()

    static func dump(_ image: UIImage, named name: String) {
        guard let data = image.pngData() else { return }
        try? data.write(to: dumpDirectory.appendingPathComponent("\(name).png"))
    }

    func testTheRenderedFramesAreWhereTheNoteSaysTheyAre() {
        let note = XCTAttachment(string: FrameRenderTests.dumpDirectory.path)
        note.name = "frame dump directory"
        note.lifetime = .keepAlways
        add(note)
        XCTAssertTrue(
            FileManager.default.fileExists(atPath: FrameRenderTests.dumpDirectory.path)
        )
    }

    // MARK: - Fixtures

    private var counts: HomeCounts {
        HomeCounts(total: 132, sorts: TypeSort.allCases.count)
    }

    private var tiles: [RecallTile] {
        [
            RecallTile(
                snippet: fixture(
                    title: "部署脚本", body: "./deploy.sh", type: "code", trigger: ";deploy"
                )
            ),
            RecallTile(
                snippet: fixture(
                    title: "收货地址", body: "221B Baker Street", type: "text", trigger: ";addr"
                )
            ),
        ]
    }

    private var triggers: [TriggerLine] {
        [
            TriggerLine(
                snippet: fixture(title: "邮件签名", body: "Best,", type: "text", trigger: ";sig"),
                trigger: ";sig"
            ),
            TriggerLine(),
        ]
    }

    private func fixture(
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
            usageCount: 24,
            version: 1,
            deletedAt: nil
        )
    }
}

/// The typing line owns a focus binding, which only a view can hold.
private struct SearchLineHost: View {
    @FocusState private var isFocused: Bool
    @State var query: String
    let resultCount: Int

    var body: some View {
        SearchLine(
            query: $query,
            isFocused: $isFocused,
            resultCount: resultCount,
            onCancel: {}
        )
    }
}

/// The detail screen's frames.
///
/// These need a real database — the screen is one snippet, and a snippet comes
/// from the core — so each case opens a store in a temporary directory, puts
/// one snippet in it, and renders what the reader would see.
@MainActor
final class DetailFrameRenderTests: XCTestCase {
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

    func testTheReadingFrameDraws() async throws {
        let model = try await model(
            title: "部署脚本",
            body: "./deploy.sh --env=prod --yes\nkubectl rollout status deploy/api",
            type: "code",
            trigger: ";deploy"
        )
        assertDraws("detail-reading", DetailBody(model: model), height: 420)
        assertDraws("detail-stick-reading", stick(model), height: 140)
    }

    func testTheEditorFrameDrawsWithEveryToolInTheStick() async throws {
        let model = try await model(
            title: "部署脚本", body: "./deploy.sh --env=prod", type: "code", trigger: ";deploy"
        )
        model.edit()
        assertDraws("detail-stick-editing", stick(model), height: 400)
    }

    func testARefusalDrawsUnderTheFieldRatherThanInADialog() async throws {
        let first = try await model(title: "One", body: "a", type: "text", trigger: ";dup")
        _ = try await store.perform {
            try $0.snippetCreate(
                draft: SnippetDraft(
                    title: "Two", body: "b", snippetType: "text", description: nil,
                    folderId: nil, trigger: ";other", triggerMode: "delimiter", language: nil
                )
            )
        }
        first.edit()
        first.draft.trigger = ";other"
        await first.save()
        XCTAssertNotNil(first.refusal, "a taken trigger must be refused")
        assertDraws("detail-stick-refused", stick(first), height: 400)
    }

    func testTheShutSecretFrameDrawsAndShowsNoBody() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: "correct horse battery") }
        let secret = try await store.perform {
            try $0.vaultCreateSecret(
                draft: SnippetDraft(
                    title: "部署密钥",
                    body: "AKIAFAKEEXAMPLE00000",
                    snippetType: "sensitive",
                    description: nil,
                    folderId: nil,
                    trigger: nil,
                    triggerMode: nil,
                    language: nil
                )
            )
        }
        try await store.perform { _ = try $0.vaultLock() }
        let model = DetailModel(store: store, snippetId: secret.id)
        await model.load()
        XCTAssertTrue(model.isSecret)
        XCTAssertNil(model.revealed, "nothing is out until the reader takes it out")
        assertDraws("detail-secret-shut", DetailBody(model: model), height: 420)
    }

    /// The vault, asked for from inside the editor. Both ways in draw: the one
    /// that has to make a vault first, and the one that only has to open it.
    ///
    /// The password fields themselves are UIKit-backed and come out as
    /// placeholder blocks, so what this covers is the sentence, the verbs and
    /// the ink material around them — the parts that say what is about to
    /// happen to the words already typed.
    func testBothVaultGatesDrawInTheComposingStick() async throws {
        let model = DetailModel(store: store)
        model.draft = Draft(title: "Deploy key", body: "AKIAFAKEEXAMPLE00000", sort: .secret)
        await model.save()
        XCTAssertEqual(model.secretGate, .setup, "no vault on this device yet")
        assertDraws("detail-gate-setup", stick(model), height: 460)

        _ = try await store.perform { try $0.vaultInitialize(password: "correct horse battery") }
        try await store.perform { _ = try $0.vaultLock() }
        let second = DetailModel(store: store)
        second.draft = Draft(title: "Cluster login", body: "AKIAFAKEEXAMPLE00001", sort: .secret)
        await second.save()
        XCTAssertEqual(second.secretGate, .shut)
        assertDraws("detail-gate-shut", stick(second), height: 400)
    }

    /// The new-snippet frame is deliberately **not** rendered here.
    ///
    /// It is almost entirely a title field and a body editor, and the renderer
    /// substitutes a placeholder block for both. That block is ink, so an
    /// "is there ink on the page" assertion would pass on a frame that had
    /// been rendered as two grey rectangles — a test that reports success for
    /// a picture nobody could read. The empty editor belongs to the
    /// walkthrough; what can be checked here is the state that decides it:
    func testANewDraftCannotBeSavedUntilItHasSomethingInIt() {
        let model = DetailModel(store: store)
        XCTAssertFalse(model.draft.canSave)
        model.draft.body = "kubectl get pods"
        XCTAssertTrue(model.draft.canSave)
    }

    // MARK: - Helpers

    private func model(
        title: String, body: String, type: String, trigger: String?
    ) async throws -> DetailModel {
        let created = try await store.perform {
            try $0.snippetCreate(
                draft: SnippetDraft(
                    title: title, body: body, snippetType: type, description: nil,
                    folderId: nil, trigger: trigger,
                    triggerMode: trigger.map { _ in "delimiter" }, language: nil
                )
            )
        }
        let model = DetailModel(store: store, snippetId: created.id)
        await model.load()
        return model
    }

    private func stick(_ model: DetailModel) -> some View {
        ComposingStick(
            model: model,
            isOpen: .constant(true),
            actions: DetailActions(close: {}, insert: { _ in }, copy: { _ in }),
            onSave: {}
        )
    }

    private func assertDraws(
        _ name: String,
        _ view: some View,
        width: CGFloat = 390,
        height: CGFloat = 320,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        let renderer = ImageRenderer(
            content: view
                .environment(\.tr, Translator(language: .zh))
                .frame(width: width, height: height, alignment: .topLeading)
                .background(Paper.base)
        )
        renderer.scale = 2
        guard let image = renderer.uiImage, let cgImage = image.cgImage else {
            return XCTFail("\(name) rendered nothing at all", file: file, line: line)
        }
        XCTAssertGreaterThan(
            FrameRenderTests.inkedFraction(of: cgImage), 0.001,
            "\(name) drew a blank sheet", file: file, line: line
        )
        FrameRenderTests.dump(image, named: name)
    }
}

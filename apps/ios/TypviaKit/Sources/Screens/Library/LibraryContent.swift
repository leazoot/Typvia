// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// The library is a type specimen book: eight kinds, eight chapters, one
/// oversized sort per page.
///
/// Every chapter is the same skeleton — mark, name and count, an undivided
/// list, the sort bar. They differ in exactly three ways, and this is where
/// those three live, so a new chapter cannot quietly acquire a fourth.
public struct ChapterStyle: Equatable, Sendable {
    /// How the body of a row is set.
    public enum Material: Equatable, Sendable {
        /// Plain paper. Most chapters.
        case plain
        /// A very faint grid under mono type. Code, and only code.
        case grid
        /// Frosted: a secret's chapter, where nothing is set at all.
        case frosted
    }

    public let material: Material
    /// One line for a trigger's worth of text, two where the content is prose.
    public let previewLines: Int
    /// The clay chapter is the only one that changes the accent; the secret
    /// chapter is the only one whose mark reverses.
    public let accent: Room
    /// A command is shown whole. The chapter is single-line content by
    /// nature, and cutting the tail off a command is cutting off the part that
    /// says which server it runs against.
    public let truncates: Bool
    /// The largest chapter is filed under the reader's own folders. It is the
    /// only one big enough for one run of rows to stop being findable.
    public let groupsByFolder: Bool

    /// The delivery summarises the chapters as differing in three ways —
    /// mark colour, body material, preview lines — and then its own
    /// chapter-by-chapter notes add three more: the template chapter counts
    /// its blanks, the command chapter does not truncate, the text chapter is
    /// grouped by folder. The notes are the specific instruction, so they win,
    /// and this type is where all of it has to live: a difference that is not
    /// stated here is a difference some screen invented.
    public static func of(_ sort: TypeSort) -> ChapterStyle {
        switch sort {
        case .code:
            ChapterStyle(
                material: .grid, previewLines: 1, accent: .library,
                truncates: true, groupsByFolder: false
            )
        case .prompt:
            ChapterStyle(
                material: .plain, previewLines: 2, accent: .library,
                truncates: true, groupsByFolder: false
            )
        case .secret:
            ChapterStyle(
                material: .frosted, previewLines: 1, accent: .vault,
                truncates: true, groupsByFolder: false
            )
        case .aiAction:
            ChapterStyle(
                material: .plain, previewLines: 1, accent: .ai,
                truncates: true, groupsByFolder: false
            )
        case .command:
            ChapterStyle(
                material: .plain, previewLines: 1, accent: .library,
                truncates: false, groupsByFolder: false
            )
        case .text:
            ChapterStyle(
                material: .plain, previewLines: 1, accent: .library,
                truncates: true, groupsByFolder: true
            )
        case .template, .link:
            ChapterStyle(
                material: .plain, previewLines: 1, accent: .library,
                truncates: true, groupsByFolder: false
            )
        }
    }
}

/// One of the reader's folders, named.
public struct FolderRef: Equatable, Sendable {
    public let id: String
    public let name: String
}

/// A run of rows under one of the reader's folders.
///
/// Only the text chapter uses these. A group is a heading and its rows — not a
/// second kind of row — so a grouped chapter and a plain one are still the
/// same list.
public struct ChapterGroup: Identifiable, Equatable, Sendable {
    /// Absent for the rows that are in no folder, which come last and carry no
    /// heading: "unfiled" is not a folder anybody made.
    public let title: String?
    public let rows: [LibraryRow]

    public var id: String { title ?? "" }

    /// Groups rows in the folders' own order, keeping each folder's rows in
    /// the order the core returned them. A folder the reader has emptied does
    /// not appear: an empty heading is a shelf label with nothing under it.
    public static func group(
        _ rows: [LibraryRow],
        folders: [(id: String, name: String)]
    ) -> [ChapterGroup] {
        var groups: [ChapterGroup] = []
        for folder in folders {
            let inFolder = rows.filter { $0.folderId == folder.id }
            guard !inFolder.isEmpty else { continue }
            groups.append(ChapterGroup(title: folder.name, rows: inFolder))
        }
        let known = Set(folders.map(\.id))
        let loose = rows.filter { row in
            guard let folder = row.folderId else { return true }
            // A folder this page did not fetch — a nested one — leaves its
            // rows visible here rather than dropping them off the chapter.
            return !known.contains(folder)
        }
        if !loose.isEmpty {
            groups.append(ChapterGroup(title: nil, rows: loose))
        }
        return groups
    }
}

/// One row in a chapter.
public struct LibraryRow: Identifiable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let trigger: String?
    public let preview: BodyPreview
    /// Which of the reader's folders this is filed under, if any.
    public let folderId: String?
    /// How many blanks a template has. The template chapter shows it; every
    /// other chapter leaves it nil, and a row whose body is withheld leaves it
    /// nil too — the number of blanks in a secret is a fact about the secret.
    public let slots: Int?

    /// - Parameters:
    ///   - sort: the chapter being read. The link chapter shows where a URL
    ///     points rather than the whole address — a list of full URLs is a list
    ///     nobody can scan — and nothing is fetched to do it.
    ///   - slots: counted by the shared template engine, never by a rule
    ///     restated here. Passing nil is what every chapter but templates does.
    init(snippet: Snippet, sort: TypeSort, slots: Int? = nil) {
        id = snippet.id
        title = snippet.title
        trigger = snippet.trigger
        folderId = snippet.folderId
        let isSensitive = snippet.securityLevel != SecurityLevelCode.normal
        if isSensitive {
            preview = .withheld
        } else if sort == .link {
            preview = .text(LibraryRow.host(of: snippet.body ?? "") ?? snippet.body ?? "")
        } else {
            preview = .text(snippet.body ?? "")
        }
        self.slots = isSensitive ? nil : slots
    }

    static func host(of body: String) -> String? {
        let trimmed = body.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let host = URL(string: trimmed)?.host else { return nil }
        return host
    }
}

/// What one chapter is showing.
public enum ChapterPhase: Equatable, Sendable {
    case loading
    /// The secret chapter with the vault shut. It is not an error and not an
    /// empty state: there is something here and it is closed.
    case locked
    case empty
    case listed
    /// The chapter could not be read. The delivery has no frame for this —
    /// its state matrix marks the library's error case as covered by the
    /// motion rules rather than drawn — so it is filled in the design's own
    /// language: one line, what is still true first, no card and no colour.
    case unavailable
}

/// One chapter's contents.
public struct Chapter: Equatable, Sendable {
    public let sort: TypeSort
    public let phase: ChapterPhase
    public let count: UInt32
    public let rows: [LibraryRow]
    /// The reader's folders, in their own order, for the one chapter that is
    /// filed under them. Empty everywhere else.
    public var folders: [FolderRef] = []
    /// There is another page behind this one. A big chapter arrives a screen
    /// at a time; asking for five hundred rows to show five is how a library
    /// starts feeling slow at exactly the size it was built for.
    public var hasMore = false

    /// The next page, landing under the rows already set. Chapters only ever
    /// grow this way — a page that replaced the list would move everything the
    /// reader is looking at.
    public func appending(_ next: [LibraryRow], hasMore moreAfter: Bool) -> Chapter {
        Chapter(
            sort: sort,
            phase: phase,
            count: count,
            rows: rows + next,
            folders: folders,
            hasMore: moreAfter
        )
    }

    /// The rows as the chapter shows them: one run, or one run per folder.
    public var groups: [ChapterGroup] {
        guard style.groupsByFolder else {
            return [ChapterGroup(title: nil, rows: rows)]
        }
        return ChapterGroup.group(rows, folders: folders.map { ($0.id, $0.name) })
    }

    public var style: ChapterStyle { ChapterStyle.of(sort) }

    /// The secret chapter's count is dots, not a number. A count is a fact
    /// about the vault's contents, and the locked chapter publishes none.
    public var showsCount: Bool { phase != .locked }

    /// The chapters, in the order the sort bar prints them.
    public static let order: [TypeSort] = TypeSort.allCases

    /// How many rows one chapter page asks for before the reader scrolls.
    /// A big library pages; it does not load eight chapters at once.
    public static let pageLimit: UInt32 = 50
}

/// Turning a page of the specimen book.
///
/// The gesture is horizontal and it commits on distance, like the frames draw
/// it: the page lifts off the sheet, the bench shows between two pages, and
/// what lands is the next chapter.
public enum ChapterPaging {
    /// The left edge belongs to the system's back gesture. A page turn that
    /// starts there is not a page turn.
    public static let systemEdge: CGFloat = 20
    /// Past this fraction of the screen, letting go turns the page.
    public static let commitFraction: CGFloat = 0.28
    /// The chapter's mark comes up last, in the closing stretch of the turn —
    /// the way a specimen page settles after the paper has already landed.
    public static let markLiftFraction = 0.4
    public static let markLift: CGFloat = 14

    /// When the mark starts rising, and for how long, within a turn of the
    /// given length.
    public static func markLiftDelay(in duration: Double) -> Double {
        duration * (1 - markLiftFraction)
    }

    public static func markLiftDuration(in duration: Double) -> Double {
        duration * markLiftFraction
    }

    public static func canBegin(atX x: CGFloat) -> Bool {
        x > systemEdge
    }

    /// Which chapter a release lands on: -1, 0 or +1 from the current one.
    public static func step(forDrag drag: CGFloat, width: CGFloat) -> Int {
        guard width > 0, abs(drag) >= width * commitFraction else { return 0 }
        return drag < 0 ? 1 : -1
    }

    /// Clamps a step to the eight chapters. The book has a first page and a
    /// last one; it does not wrap, because wrapping hides which end you are at.
    public static func chapter(after index: Int, step: Int, count: Int) -> Int {
        min(max(index + step, 0), count - 1)
    }
}

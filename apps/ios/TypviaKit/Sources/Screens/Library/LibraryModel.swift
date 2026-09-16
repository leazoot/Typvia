// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// The library's state: which chapter is open, and what is in it.
///
/// One chapter is read at a time. The specimen book has eight pages and a
/// reader looks at one of them; loading the other seven to be ready would cost
/// eight queries for seven pages nobody asked for.
@MainActor
public final class LibraryModel: ObservableObject {
    @Published public private(set) var chapter: Chapter
    @Published public private(set) var index: Int

    private let store: TypviaStore
    private var loadTask: Task<Void, Never>?
    private var isPaging = false

    public init(store: TypviaStore, opening sort: TypeSort = .text) {
        self.store = store
        index = Chapter.order.firstIndex(of: sort) ?? 0
        chapter = Chapter(sort: sort, phase: .loading, count: 0, rows: [])
    }

    public var sort: TypeSort { Chapter.order[index] }

    /// Opens a chapter. Called on appearance, on a page turn and on a tap in
    /// the sort bar, and it always starts from the loading frame — the reader
    /// asked for a different page, so the old page's rows are not an answer.
    public func open(_ newIndex: Int) {
        let bounded = min(max(newIndex, 0), Chapter.order.count - 1)
        index = bounded
        let sort = Chapter.order[bounded]
        chapter = Chapter(sort: sort, phase: .loading, count: 0, rows: [])
        loadTask?.cancel()
        loadTask = Task { [weak self] in
            guard let self else { return }
            let loaded = await read(sort)
            guard !Task.isCancelled, sort == self.sort else { return }
            chapter = loaded
        }
    }

    /// Reads the next page of the open chapter. Called when the last row
    /// appears; a second call while one is in flight is ignored, so a fast
    /// scroll asks once.
    public func loadMore() async {
        guard chapter.hasMore, !isPaging else { return }
        isPaging = true
        defer { isPaging = false }
        let sort = chapter.sort
        let offset = UInt32(chapter.rows.count)
        let next = try? await store.perform { core -> [LibraryRow] in
            if sort == .secret {
                return try core.vaultList(limit: Chapter.pageLimit, offset: offset)
                    .map { LibraryRow(snippet: $0, sort: sort) }
            }
            return try core.snippetListPage(
                view: "all",
                folderId: nil,
                snippetType: sort.coreType,
                limit: Chapter.pageLimit,
                offset: offset
            )
            .map { LibraryRow(snippet: $0, sort: sort, slots: LibraryModel.slots($0, sort, core)) }
        }
        // A page that failed to arrive leaves the chapter as it was, still
        // saying there is more: the reader can scroll again.
        guard let next, !next.isEmpty, sort == chapter.sort else { return }
        chapter = chapter.appending(next, hasMore: next.count == Int(Chapter.pageLimit))
    }

    public func turn(by step: Int) {
        guard step != 0 else { return }
        open(
            ChapterPaging.chapter(after: index, step: step, count: Chapter.order.count)
        )
    }

    private func read(_ sort: TypeSort) async -> Chapter {
        let result = try? await store.perform { core -> Chapter in
            // The secret chapter is the vault's list, not the library's, and
            // a shut vault answers with nothing at all — no rows and no count.
            if sort == .secret {
                let vault = try core.vaultStatus()
                // No vault yet is not a locked door. A reader who has never
                // made one should be told where secrets live, not asked to
                // open something that does not exist.
                guard vault.initialized else {
                    return Chapter(sort: sort, phase: .empty, count: 0, rows: [])
                }
                guard vault.unlocked else {
                    return Chapter(sort: sort, phase: .locked, count: 0, rows: [])
                }
                let rows = try core.vaultList(limit: Chapter.pageLimit, offset: 0)
                    .map { LibraryRow(snippet: $0, sort: sort) }
                return Chapter(
                    sort: sort,
                    phase: rows.isEmpty ? .empty : .listed,
                    count: UInt32(rows.count),
                    rows: rows,
                    // The vault reports no total, so a full page is the only
                    // sign there may be another.
                    hasMore: rows.count == Int(Chapter.pageLimit)
                )
            }
            let coreType = sort.coreType
            let count = try core.snippetCount(view: "all", folderId: nil, snippetType: coreType)
            // Only the grouped chapter asks for folders, and it asks once.
            let folders = ChapterStyle.of(sort).groupsByFolder
                ? (try? core.folderListChildren(parentId: nil))?
                    .map { FolderRef(id: $0.id, name: $0.name) } ?? []
                : []
            let rows = try core.snippetListPage(
                view: "all",
                folderId: nil,
                snippetType: coreType,
                limit: Chapter.pageLimit,
                offset: 0
            )
            .map { LibraryRow(snippet: $0, sort: sort, slots: LibraryModel.slots($0, sort, core)) }
            return Chapter(
                sort: sort,
                phase: count == 0 ? .empty : .listed,
                count: count,
                rows: rows,
                folders: folders,
                hasMore: rows.count < Int(count)
            )
        }
        // A chapter that could not be read says so. Reporting it as empty
        // would tell the reader their snippets are gone.
        return result ?? Chapter(sort: sort, phase: .unavailable, count: 0, rows: [])
    }
}

extension LibraryModel {
    /// How many blanks a template row has, asked of the shared template engine
    /// rather than found with a pattern here. It parses the body it was handed
    /// and touches no database, so a page of templates still costs one query.
    nonisolated static func slots(_ snippet: Snippet, _ sort: TypeSort, _ core: TypviaCore) -> Int? {
        guard sort == .template, let body = snippet.body else { return nil }
        return (try? core.templateVariables(body: body))?.count
    }
}

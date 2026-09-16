// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// The home screen's state.
///
/// It holds no rules. Ranking, sensitivity, trigger validity and what counts
/// as recent are all answers the core gives; this asks for them, keeps the
/// latest answer, and says which of the six frames that answer belongs to.
///
/// Searching replaces rather than accumulates: each keystroke starts a fresh
/// query and cancels the one before it, and the results that arrive are the
/// whole list. There is no debounce — the design's instruction is that the
/// screen answers every keystroke, and a local index is fast enough to mean it.
@MainActor
public final class HomeModel: ObservableObject {
    @Published public private(set) var phase: HomePhase = .loading
    @Published public private(set) var counts = HomeCounts(total: 0, sorts: TypeSort.allCases.count)
    @Published public private(set) var tiles: [RecallTile] = []
    @Published public private(set) var triggers: [TriggerLine] = []
    @Published public private(set) var results: [SearchResult] = []
    @Published public private(set) var notice: HomeNotice?
    /// When sync last got through, if it ever has. Reported next to the counts
    /// whether or not anything is wrong.
    @Published public private(set) var lastSyncAt: Int64?
    @Published public private(set) var isRetrying = false

    /// What the reader has typed. Assigning it starts a search.
    @Published public var query: String = "" {
        didSet {
            guard query != oldValue else { return }
            search()
        }
    }

    private let store: TypviaStore
    private var isLoaded = false
    private var searchTask: Task<Void, Never>?

    public init(store: TypviaStore) {
        self.store = store
    }

    /// Loads the shelf. Safe to call again — a returning reader gets fresh
    /// counts without the skeleton coming back, because the phase only falls
    /// back to loading before the first answer.
    public func load() async {
        do {
            let shelf = try await readShelf()
            counts = HomeCounts(total: shelf.total, sorts: TypeSort.allCases.count)
            tiles = shelf.tiles
            triggers = shelf.triggers
            lastSyncAt = shelf.lastSyncAt
            notice = shelf.queued > 0
                ? .syncPaused(
                    queued: shelf.queued,
                    lastSyncAt: shelf.lastSyncAt,
                    reason: shelf.syncFailure
                )
                : nil
            isLoaded = true
        } catch {
            // The shelf keeps whatever it last had: a failed refresh is not a
            // reason to take a working list away from the reader.
            notice = .loadFailed
        }
        updatePhase()
    }

    /// The notice card's one action, which is whatever that notice is about:
    /// a queue that has not gone out gets a sync round, a read that failed
    /// gets read again. Offering to retry sync when the database would not
    /// open would be a button aimed at the wrong problem.
    public func retry() async {
        guard !isRetrying else { return }
        isRetrying = true
        defer { isRetrying = false }
        if case .syncPaused = notice {
            do {
                _ = try await store.perform { try $0.syncNow() }
                // Rows that arrived have to reach the document the keyboard
                // reads; nobody else will rewrite it while the reader is here.
                await SnapshotPublish.run(store: store)
            } catch {
                // Staying paused is the honest outcome and the card already
                // says so. What must not happen is the card vanishing.
            }
        }
        await load()
    }

    private func search() {
        searchTask?.cancel()
        let query = query
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else {
            results = []
            updatePhase()
            return
        }
        updatePhase()
        searchTask = Task { [weak self] in
            guard let self else { return }
            let rows = (try? await store.perform { core in
                try core.searchAll(query: trimmed, limit: SearchResult.pageLimit)
                    .map { SearchResult(snippet: $0, query: trimmed) }
            }) ?? []
            guard !Task.isCancelled else { return }
            // Whole-list replacement, on purpose: a result set that animates
            // from the previous one asks the reader to watch rows move
            // instead of reading them.
            results = rows
        }
    }

    private func updatePhase() {
        phase = HomePhase.resolve(isLoaded: isLoaded, total: counts.total, query: query)
    }

    private func readShelf() async throws -> HomeShelf {
        try await store.perform { core in
            let counts = try core.libraryCounts()
            let recent = try core.snippetListPage(
                view: "recent",
                folderId: nil,
                snippetType: nil,
                limit: HomeShelf.recentLimit,
                offset: 0
            )
            // The trigger lines are headed "the ones you type most", so they
            // ask for that order. Only as many as the shelf prints: this is a
            // second query on the load path and it pays for exactly what it
            // shows.
            let mostUsed = try core.snippetListPage(
                view: "used",
                folderId: nil,
                snippetType: nil,
                limit: HomeShelf.triggerPageLimit,
                offset: 0
            )
            let vault = try core.vaultStatus()
            // Sync is optional equipment. A device where it was never set up,
            // or where the secure store is unavailable, has no queue to report
            // and must not be told it has a problem.
            let sync = try? core.syncStatus()
            return HomeShelf.assemble(
                total: counts.total,
                recent: recent,
                mostUsed: mostUsed,
                vaultLocked: vault.initialized && !vault.unlocked,
                queued: sync.map { $0.enabled ? $0.pendingBacklog : 0 } ?? 0,
                lastSyncAt: sync?.lastSyncAt,
                syncFailure: sync?.lastFailure
            )
        }
    }
}

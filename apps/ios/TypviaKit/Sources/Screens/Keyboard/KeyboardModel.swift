// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// What the bench is holding.
///
/// The keyboard reads the snapshot the app wrote into the shared container and
/// nothing else. It does not open the database — one process owns the single
/// writer — and every question about *which* snippets match is put to the
/// shared engine rather than answered here.
@MainActor
public final class KeyboardModel: ObservableObject {
    @Published public private(set) var phase: KeyboardPhase = .browsing
    @Published public private(set) var tiles: [KeyboardTile] = []
    @Published public private(set) var reach = KeyboardReach(available: 0, total: 0)

    @Published public var query: String = "" {
        didSet {
            guard query != oldValue else { return }
            filter()
        }
    }

    private var json: String?
    /// The undo bar's own life. Cancelled the moment anything else happens.
    private var noteTask: Task<Void, Never>?
    private var spentId: String?

    public init() {}

    /// Reads the snapshot. Called when the keyboard appears — it is cheap, and
    /// the alternative is a bench showing what the library looked like the
    /// last time the reader summoned it.
    public func load() {
        apply(SharedContainer.snapshotJSON())
    }

    /// The half of `load` that does not need a shared container, so the state
    /// it produces can be checked against a document rather than against a
    /// device.
    func apply(_ document: String?) {
        json = document
        guard let json else {
            // No snapshot at all: the app has never written one, or this
            // process cannot see the container. Either way the honest frame
            // is the one that says what is reachable, which is nothing.
            reach = KeyboardReach(available: 0, total: 0)
            tiles = []
            phase = .limited(available: 0, total: 0)
            return
        }
        let entries = (try? snapshotEntries(json: json)) ?? []
        // The denominator is the snapshot's own entry count, which is what
        // this keyboard can reach — not the app's library total, which it has
        // no way to check and no business printing.
        let total = (try? parseSnapshot(json: json).entryTotal).map(Int.init) ?? entries.count
        reach = KeyboardReach(available: entries.count, total: total)
        tiles = makeTiles(entries)
        phase = .browsing
    }

    /// Every keystroke re-asks the engine. There is no debounce and no local
    /// matching: which snippets a query hits is a rule, and rules live in one
    /// place.
    private func filter() {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        guard let json else { return }
        guard !trimmed.isEmpty else {
            tiles = makeTiles((try? snapshotEntries(json: json)) ?? [])
            phase = .browsing
            return
        }
        let hits = (try? filterEntries(json: json, query: trimmed)) ?? []
        tiles = makeTiles(hits)
        phase = hits.isEmpty ? .noMatch(trimmed) : .filtering(trimmed)
    }

    /// The body to type, asked for one at a time. A secret's body never comes
    /// back through here — the snapshot holds no plaintext for it, and the
    /// panel routes those to the ink room instead.
    public func body(for id: String) -> String? {
        guard let json, let tile = tiles.first(where: { $0.id == id }), !tile.isSecret else {
            return nil
        }
        return (try? entryBody(json: json, id: id)) ?? nil
    }

    /// Records what was just used, so the bench can step it back rather than
    /// reshuffling under the thumb that tapped it.
    public func markUsed(_ id: String, title: String) {
        spentId = id
        if let json {
            let source = query.trimmingCharacters(in: .whitespaces).isEmpty
                ? (try? snapshotEntries(json: json)) ?? []
                : (try? filterEntries(json: json, query: query)) ?? []
            tiles = makeTiles(source)
        }
        phase = .inserted(title: title)
        // The undo bar lives four seconds and then gets out of the way. It was
        // written down as a number and never acted on, so the bar stayed on a
        // keyboard whose 118 points of tile run it was covering — the reader
        // had to type something to get their bench back.
        noteTask?.cancel()
        noteTask = Task { [weak self] in
            try? await Task.sleep(
                nanoseconds: UInt64(InsertedNote.life * 1_000_000_000)
            )
            guard !Task.isCancelled else { return }
            guard case .inserted = self?.phase else { return }
            self?.backToBench()
        }
    }

    public func openSecret(_ id: String, title: String) {
        phase = .secret(title: title)
    }

    public func backToBench() {
        noteTask?.cancel()
        noteTask = nil
        phase = query.trimmingCharacters(in: .whitespaces).isEmpty ? .browsing : .filtering(query)
    }

    /// The first hit is the lead: it is wider and carries a preview, because
    /// the compositor hands over the one most likely wanted.
    private func makeTiles(_ entries: [SnapshotEntry]) -> [KeyboardTile] {
        entries.enumerated().map { index, entry in
            KeyboardTile(
                entry: entry,
                isLead: index == 0 && !query.trimmingCharacters(in: .whitespaces).isEmpty,
                isSpent: entry.id == spentId
            )
        }
    }
}

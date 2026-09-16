// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// What the home screen shows, as values.
///
/// Everything here is derived from what the core returned and nothing here
/// decides anything the core decides: no ranking, no sensitivity test of its
/// own, no trigger parsing. The one rule these types do hold is a display
/// rule with teeth — a secret has no previewable body, and that is expressed
/// by leaving no way to build a row that carries one.

/// A query's overlap with a trigger word, split so the matched head can be
/// underlined and the tail cannot.
///
/// Which snippets matched is the core's answer; this only re-finds the head of
/// a trigger the reader has already typed, in order to draw it.
public struct TriggerMatch: Equatable, Sendable {
    public let matched: String
    public let rest: String

    public init(trigger: String, query: String) {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty,
              let range = trigger.range(of: trimmed, options: [.anchored, .caseInsensitive])
        else {
            matched = ""
            rest = trigger
            return
        }
        matched = String(trigger[range])
        rest = String(trigger[range.upperBound...])
    }
}

/// A recall card: one of the sheets in the horizontal run under the search
/// position.
public struct RecallTile: Identifiable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let sort: TypeSort?
    public let trigger: String?
    public let usageCount: UInt64

    init(snippet: Snippet) {
        id = snippet.id
        title = snippet.title
        sort = TypeSort(coreType: snippet.snippetType)
        trigger = snippet.trigger
        usageCount = snippet.usageCount
    }
}

/// A line in the trigger list under the cards.
public struct TriggerLine: Identifiable, Equatable, Sendable {
    /// A locked vault is a line in this list rather than a banner: the reader
    /// is told their secrets are present and closed, in the place they would
    /// have been listed.
    public enum Kind: Equatable, Sendable {
        case snippet(trigger: String)
        case lockedVault
    }

    public let id: String
    public let kind: Kind
    /// Empty for the vault line, which is a state rather than a snippet: its
    /// sentence is the view's, in the reader's language.
    public let title: String
    public let sort: TypeSort?

    init(snippet: Snippet, trigger: String) {
        id = snippet.id
        kind = .snippet(trigger: trigger)
        title = snippet.title
        sort = TypeSort(coreType: snippet.snippetType)
    }

    init(lockedVault: Void = ()) {
        id = "vault.locked"
        kind = .lockedVault
        title = ""
        sort = .secret
    }
}

/// One search result row: mark, title, preview, and whatever sits at the end.
public struct SearchResult: Identifiable, Equatable, Sendable {
    /// The row's tail. A secret gets a verification note where a trigger
    /// would be, because inserting one is not a thing that happens from here.
    public enum Tail: Equatable, Sendable {
        case trigger(TriggerMatch)
        case needsVerification
        case none
    }

    public let id: String
    public let title: String
    public let sort: TypeSort?
    public let preview: BodyPreview
    public let tail: Tail

    /// How many rows one query asks for. Past this the reader is refining the
    /// query, not scrolling.
    public static let pageLimit: UInt32 = 30

    /// The only way to build a row. A sensitive snippet gets `withheld` and a
    /// verification tail no matter what body it arrived with — the bridge
    /// already withholds it, and this makes a regression on that side visible
    /// here instead of printing it.
    init(snippet: Snippet, query: String) {
        id = snippet.id
        title = snippet.title
        sort = TypeSort(coreType: snippet.snippetType)
        let isSensitive = snippet.securityLevel != SecurityLevelCode.normal
        preview = isSensitive ? .withheld : .text(snippet.body ?? "")
        if isSensitive {
            tail = .needsVerification
        } else if let trigger = snippet.trigger, !trigger.isEmpty {
            tail = .trigger(TriggerMatch(trigger: trigger, query: query))
        } else {
            tail = .none
        }
    }
}

/// The two numbers under the search rule.
public struct HomeCounts: Equatable, Sendable {
    public let total: UInt32
    /// How many kinds the library files snippets under. It is the sort
    /// vocabulary's size, not a count of which kinds happen to be in use —
    /// the chapters exist whether or not they are occupied.
    public let sorts: Int
}

/// The one card on this screen that carries bad news.
///
/// The design gives home a single frame for both "offline" and "error", so
/// both arrive here. Every case is written the same way round: what still
/// works, then what does not, then what the reader can do.
public enum HomeNotice: Equatable, Sendable {
    /// Changes are waiting in the outbox.
    ///
    /// - Parameter reason: why the last round stopped, when one has. It is a
    ///   category from the engine, never a diagnosis made here: a queue can be
    ///   non-empty simply because no round has run yet, and in that case there
    ///   is no cause to state and the card does not invent one.
    case syncPaused(queued: UInt64, lastSyncAt: Int64?, reason: SyncFailureKind?)
    /// The local library could not be read.
    case loadFailed
}

/// Everything the shelf shows, assembled in one pass off the main thread.
///
/// The assembly happens here rather than in the model so that it runs inside
/// the one call that already holds the database, and so that it can be tested
/// without a screen.
public struct HomeShelf: Equatable, Sendable {
    public let total: UInt32
    public let tiles: [RecallTile]
    public let triggers: [TriggerLine]
    public let queued: UInt64
    public let lastSyncAt: Int64?
    /// Why the last round stopped, straight from the bridge.
    public let syncFailure: SyncFailureKind?

    /// How many rows each part of the shelf asks for. The frames draw three
    /// cards and three trigger lines; these leave room to scroll without
    /// asking the database for a page nobody will reach.
    public static let recentLimit: UInt32 = 12
    static let triggerLimit = 3
    /// Asked of the usage-ordered view. A margin over `triggerLimit`, because
    /// the most-used snippets need not all have a trigger word — without it a
    /// heavily used snippet with no trigger would silently shorten the list.
    public static let triggerPageLimit: UInt32 = 12

    /// - Parameters:
    ///   - recent: the core's recall order, used as given. The cards are this
    ///     list, in this order.
    ///   - mostUsed: the core's usage order. The trigger lines are headed
    ///     *the ones you type most*, so they come from the list that answers
    ///     that — not from the recent one, which answers a different question
    ///     and would put a heading over the wrong rows. Both orders are the
    ///     core's; neither is re-sorted here.
    static func assemble(
        total: UInt32,
        recent: [Snippet],
        mostUsed: [Snippet] = [],
        vaultLocked: Bool,
        queued: UInt64,
        lastSyncAt: Int64?,
        syncFailure: SyncFailureKind? = nil
    ) -> HomeShelf {
        var triggers = mostUsed
            .compactMap { snippet -> TriggerLine? in
                guard let trigger = snippet.trigger, !trigger.isEmpty else { return nil }
                return TriggerLine(snippet: snippet, trigger: trigger)
            }
            .prefix(triggerLimit)
            .map { $0 }
        // A locked vault closes the list: the reader is told the secrets are
        // there and shut, in the place they would otherwise have been listed.
        if vaultLocked {
            triggers.append(TriggerLine())
        }
        return HomeShelf(
            total: total,
            tiles: recent.map(RecallTile.init(snippet:)),
            triggers: triggers,
            queued: queued,
            lastSyncAt: lastSyncAt,
            syncFailure: syncFailure
        )
    }
}

/// Which of the six frames the screen is currently showing.
public enum HomePhase: Equatable, Sendable {
    case loading
    case empty
    case resting
    case searching

    /// The order matters: nothing is empty until it has finished loading, and
    /// a query outranks the shelf because the reader is looking at results.
    public static func resolve(isLoaded: Bool, total: UInt32, query: String) -> HomePhase {
        if !query.trimmingCharacters(in: .whitespaces).isEmpty { return .searching }
        if !isLoaded { return .loading }
        return total == 0 ? .empty : .resting
    }
}

/// The core's `security_level` vocabulary, as the two strings that cross the
/// bridge. Anything that is not `normal` is treated as sensitive: an unknown
/// value must fail closed, never open.
enum SecurityLevelCode {
    static let normal = "normal"
}

extension TypeSort {
    /// Maps the core's snippet type to the sort that stands for it.
    ///
    /// Returns nil for a type this design has no sort for — `temporary`
    /// today, and whatever a newer database adds tomorrow. A row without a
    /// sort draws no mark rather than borrowing another kind's, so a value
    /// this version does not understand cannot end up mislabelled as one it
    /// does.
    public init?(coreType: String) {
        switch coreType {
        case "text": self = .text
        case "code": self = .code
        case "command": self = .command
        case "prompt": self = .prompt
        case "template": self = .template
        case "sensitive": self = .secret
        case "ai_action": self = .aiAction
        case "link": self = .link
        default: return nil
        }
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// The recycle bin.
///
/// The delivery draws no frame for it — it is one of the things the Tauri
/// shell had and the rewrite has to bring back — so it is set in the design's
/// own language: the data chapter's page, rows like the library's, one clay
/// label for the irreversible half, and no dialog anywhere.

/// One thing waiting to be either put back or let go.
public struct TrashRow: Identifiable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let sort: TypeSort?
    /// When it was thrown away. What the row reports is how long is left, not
    /// how long ago it went — the useful fact is the deadline.
    public let deletedAt: Int64?

    init(snippet: Snippet) {
        id = snippet.id
        title = snippet.title
        sort = TypeSort(coreType: snippet.snippetType)
        deletedAt = snippet.deletedAt
    }
}

/// How long a thrown-away snippet is kept before the app clears it.
///
/// The window is the core's, not this screen's; what is here is how to say it.
public enum TrashWindow {
    /// The retention the core enforces, in days.
    ///
    /// Read from the engine, not restated here. The rule belongs to the core;
    /// a copy of it on this side is a copy that can drift, and a drifted copy
    /// means this screen states a deadline that is not the one being kept.
    public static let days = Int(trashRetentionDays())

    /// Days left, given when it went and when now is. Never negative: a row
    /// past its window is one the next purge will take, and "−3 days" is not
    /// something to print at a reader.
    public static func daysLeft(deletedAt: Int64?, now: Int64) -> Int? {
        guard let deletedAt else { return nil }
        let elapsed = Double(now - deletedAt) / 86_400_000
        return max(days - Int(elapsed.rounded(.down)), 0)
    }
}

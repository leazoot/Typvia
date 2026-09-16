// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// The vault, as values.
///
/// The shut vault publishes nothing: not the contents, not the count, not how
/// many attempts are left. What it says is that the things in it are on this
/// device, and that is the whole of it.

/// Which way in was tried.
///
/// The core answers with a kind of refusal and has no idea which door the
/// reader pressed; this room does, and needs both to say anything true. A
/// refusal alone cannot tell a wrong master password from a Face ID that was
/// never given a key to hold.
public enum VaultDoor: Equatable, Sendable {
    case faceId
    case masterPassword
}

/// What the vault room is showing.
public enum VaultPhase: Equatable, Sendable {
    /// Shut. Seven dots where a count would be.
    case shut
    /// Opening: the one 320ms moment in the product.
    case opening
    case open
    /// A try that did not open it. Not a punishment, and not a countdown of
    /// remaining tries — there is no such countdown. It carries the door
    /// because the sentence depends on it.
    case didNotOpen(VaultDoor)
    /// No vault has been made on this device yet.
    case absent
}

/// One secret, listed. A name and when it was last taken out — never a body,
/// and never a preview of one.
public struct VaultEntry: Identifiable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let lastUsedAt: Int64?

    /// How many rows one read asks for. A vault with more than this is a
    /// vault that needs a paging story of its own.
    public static let pageLimit: UInt32 = 100

    init(snippet: Snippet) {
        id = snippet.id
        title = snippet.title
        lastUsedAt = snippet.lastUsedAt
    }
}

/// The idle window, as a rule that shrinks.
///
/// Leaving the screen, switching apps, a screenshot, or five minutes of
/// stillness all shut it again. The countdown is a line rather than digits: it
/// should be visible out of the corner of an eye without hurrying anyone.
public struct RelockClock: Equatable, Sendable {
    public let idleTimeoutMs: Int64
    public let secondsLeft: TimeInterval

    /// How much of the line is still there.
    public var remaining: Double {
        let window = Double(idleTimeoutMs) / 1000
        guard window > 0 else { return 0 }
        return min(max(secondsLeft / window, 0), 1)
    }

    /// Minutes and seconds, for the one place a number is allowed: the line
    /// says *how much*, this says *how long* — and the design puts it beside
    /// the line rather than in place of it.
    public var clockText: String {
        let total = Int(max(secondsLeft, 0).rounded())
        return String(format: "%d:%02d", total / 60, total % 60)
    }
}

/// How the vault list is ordered: by when each secret was last taken out,
/// most recent first, with the never-taken ones after them.
///
/// The core has no view for this, so the order is stated here — and it is an
/// ordering of names and timestamps, never of content.
public enum VaultOrder {
    public static func sort(_ entries: [VaultEntry]) -> [VaultEntry] {
        entries.sorted { first, second in
            switch (first.lastUsedAt, second.lastUsedAt) {
            case let (lhs?, rhs?): return lhs > rhs
            case (nil, _?): return false
            case (_?, nil): return true
            case (nil, nil): return first.title < second.title
            }
        }
    }
}

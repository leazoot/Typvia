// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import CoreGraphics
import Foundation

/// The keyboard is a composing bench 260 points tall.
///
/// Everything here is measured against that height, because height is the
/// most expensive thing a keyboard has: the search row gets no border, only a
/// caret and a hairline, and nothing is drawn that a reader would not use.

/// The four bands the panel is divided into.
public enum KeyboardBand {
    public static let search: CGFloat = 44
    public static let sorts: CGFloat = 36
    public static let tiles: CGFloat = 118
    public static let functions: CGFloat = 44

    /// The four bands come to 242 of the 260 the keyboard asks the system
    /// for. The remaining strip is the home indicator's, and the panel leaves
    /// it alone rather than stretching a band into it — a function row that
    /// runs under the indicator is a function row people mis-tap.
    public static var bands: CGFloat { search + sorts + tiles + functions }
    public static var gutter: CGFloat { total - bands }
    /// What the panel occupies, and what the controller asks for. One number,
    /// so the two cannot drift.
    public static let total = Tokens.Viewport.keyboardHeight
}

/// What the panel is doing.
public enum KeyboardPhase: Equatable, Sendable {
    /// The bench at rest: recent tiles, all kinds.
    case browsing
    /// A trigger is being typed; the tiles are filtered.
    case filtering(String)
    /// Something was just typed into the host app, and can be undone for a
    /// few seconds.
    case inserted(title: String)
    /// A secret was chosen: the whole panel becomes an ink room and does one
    /// thing at a time.
    case secret(title: String)
    /// A query with nothing behind it. The empty state is also a way in.
    case noMatch(String)
    /// The extension cannot reach the library. What it can still do is said
    /// first.
    case limited(available: Int, total: Int)
}

/// One snippet on the bench.
public struct KeyboardTile: Identifiable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let sort: TypeSort?
    public let trigger: String?
    public let isSecret: Bool
    /// The first hit widens and shows two lines of body — the compositor
    /// hands over the one most likely wanted.
    public let isLead: Bool
    /// Just used: it steps back rather than disappearing.
    public let isSpent: Bool

    init(entry: SnapshotEntry, isLead: Bool = false, isSpent: Bool = false) {
        id = entry.id
        // A secret reaches the keyboard as an id and an envelope: the snapshot
        // carries no title for it, and a sheet drawn from that was a sheet
        // with nothing written on it. The dot run says there is something
        // there without saying anything about it — the same stand-in the
        // library and the vault use, and the same one the other platform
        // draws.
        title = entry.title.isEmpty ? BodyPreview.shortMask : entry.title
        sort = entry.isSensitive ? .secret : TypeSort(coreType: entry.snippetType)
        trigger = entry.trigger
        isSecret = entry.isSensitive
        self.isLead = isLead
        self.isSpent = isSpent
    }

    /// Widths from the frames: the leading tile is wider because it carries a
    /// preview the others do not.
    public static let width: CGFloat = 134
    public static let leadWidth: CGFloat = 190
}

/// Typing into someone else's text field, one character at a time, because
/// that is what the product does — it does not paste.
public enum TypeIn {
    /// The delivery's rate, and its cap: past this a long body simply lands,
    /// since watching four seconds of animation is not a feature.
    public static func duration(characterCount: Int) -> Double {
        min(
            Double(characterCount) * Tokens.Motion.typeInPerCharacter,
            Tokens.Motion.typeInCap
        )
    }

    /// How a body actually goes in.
    public enum Plan: Equatable, Sendable {
        /// All at once. Not a fallback — for a long body it is the right
        /// answer, and it is what the delivery means by a long text dropping
        /// straight in.
        case atOnce
        /// One character every `interval` seconds.
        case character(interval: Double)
    }

    /// The shortest gap worth making. Below it the characters arrive faster
    /// than the host app can draw them, so the motion is invisible and all
    /// that is left is a few hundred edits to somebody else's text field.
    static let shortestInterval = 0.004

    public static func plan(characterCount: Int) -> Plan {
        guard characterCount > 1 else { return .atOnce }
        let interval = duration(characterCount: characterCount) / Double(characterCount)
        return interval < shortestInterval ? .atOnce : .character(interval: interval)
    }
}

/// What the panel says when it has no document to read.
///
/// Kept out of the view so it can be read by something other than a person
/// looking at a phone. The sentences these replaced told the reader to turn on
/// full access — a switch this keyboard declares it does not want, so iOS does
/// not draw it at all — and they survived for months because a sentence inside
/// a view is a sentence no test can reach.
public enum KeyboardPanelCopy {
    /// What is still true. Nothing has been lost.
    public static func nothingToRead(_ tr: Translator) -> String {
        tr(
            "Your snippets are all still there — this keyboard just cannot see them yet.",
            "你的片段都还在,只是这个键盘还看不到它们。"
        )
    }

    /// The act that fixes the cause the reader can fix. The other cause — a
    /// shared container this process cannot reach — is not distinguishable
    /// from in here, so neither is named.
    public static func openTheAppOnce(_ tr: Translator) -> String {
        tr(
            "Open Typvia once and it writes out the copy this keyboard reads.",
            "打开一次 Typvia,它会把这个键盘要读的那一份写出来。"
        )
    }
}

/// How much of the library the extension can actually see.
///
/// What this keyboard has is whatever the app wrote out for it. It asks for no
/// permission to get it: the document is in the shared container, and reaching
/// that is not something a reader can switch on or off.
public struct KeyboardReach: Equatable, Sendable {
    public let available: Int
    public let total: Int

    public init(available: Int, total: Int) {
        self.available = available
        self.total = total
    }

    public var isComplete: Bool { available >= total }
}

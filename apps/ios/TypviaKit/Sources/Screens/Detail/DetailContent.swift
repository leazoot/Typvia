// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// One snippet, open.
///
/// The body is the whole point of this screen, so the tools go to the bottom
/// and the type of the snippet is said by the material the body sits on rather
/// than by a label repeating what the mark already says.

/// What the reader is doing here.
public enum DetailMode: Equatable, Sendable {
    case reading
    case editing
    /// A snippet that does not exist yet.
    case creating
}

/// A snippet being written. The editor holds one of these; nothing is saved
/// until the reader says so.
public struct Draft: Equatable, Sendable {
    public var title: String
    public var body: String
    public var sort: TypeSort?
    public var trigger: String
    public var folderId: String?

    public init(
        title: String = "",
        body: String = "",
        sort: TypeSort? = nil,
        trigger: String = "",
        folderId: String? = nil
    ) {
        self.title = title
        self.body = body
        self.sort = sort
        self.trigger = trigger
        self.folderId = folderId
    }

    public init(snippet: Snippet) {
        title = snippet.title
        body = snippet.body ?? ""
        sort = TypeSort(coreType: snippet.snippetType)
        trigger = snippet.trigger ?? ""
        folderId = snippet.folderId
    }

    /// Saving needs something to save. A title alone is a name for nothing, so
    /// the body is what counts — and a new snippet with no kind chosen is
    /// fine, because the kind is decided from the content after it is saved
    /// rather than demanded before.
    public var canSave: Bool {
        !body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !needsAName
    }

    /// A secret has to be named by hand.
    ///
    /// Everything else may go unnamed — the shared layer files it under its own
    /// first line. A secret may not: **a title is searchable**, so borrowing the
    /// first line of a secret would put the secret itself in the index. The
    /// vault refuses an unnamed one; this asks for the name rather than sending
    /// something that will come back refused with nothing on screen to explain
    /// it.
    public var needsAName: Bool {
        effectiveSort == .secret
            && title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    /// The kind a draft is filed under while none has been chosen. Text is the
    /// plainest of the eight and the one that claims least.
    public var effectiveSort: TypeSort { sort ?? .text }

    /// The kinds the editor may file a draft under.
    ///
    /// All eight while writing something new. An existing snippet loses the
    /// secret one: turning what is already saved into a secret means
    /// encrypting it and dropping the plaintext its own history has been
    /// keeping, which is a different act from choosing what a thing is — and
    /// an ordinary save is refused for that kind behind the bridge anyway.
    public static func offeredKinds(isNew: Bool) -> [TypeSort] {
        isNew ? Chapter.order : Chapter.order.filter { $0 != .secret }
    }
}

/// What the clipboard is offering the new-snippet screen.
///
/// Only something the reader copied a moment ago is offered. An hour-old
/// clipboard is not an offer, it is a surprise — and the card is never
/// permanent furniture.
public struct ClipboardOffer: Equatable, Sendable {
    public let text: String
    public let age: TimeInterval

    /// How recent a copy has to be to be worth showing.
    public static let freshWindow: TimeInterval = 60

    public var isFresh: Bool { age >= 0 && age <= ClipboardOffer.freshWindow }
}

/// A secret's body, and how long it stays out.
///
/// The body exists on this screen only between the moment it is fetched and
/// the moment it goes back under. It is never held anywhere else, and nothing
/// keeps a copy for the next time the screen is opened.
/// The plaintext itself is deliberately *not* carried here. It lives in the
/// screen's own short-lived state, so a value that outlives a frame cannot end
/// up holding a decrypted secret.
public enum Reveal: Equatable, Sendable {
    case hidden
    case shown(secondsLeft: TimeInterval)

    /// How long a revealed secret stays visible.
    public static let window: TimeInterval = 20

    /// The countdown, as the fraction of the underline still to run. A shrinking
    /// rule rather than a number: a digit ticking down is a stopwatch, and a
    /// stopwatch on a secret reads as a threat.
    public static func remaining(secondsLeft: TimeInterval) -> Double {
        min(max(secondsLeft / window, 0), 1)
    }
}

/// What stands between a secret that has been written and a secret that has
/// been stored.
///
/// A secret is not saved the way the other seven kinds are: its body is
/// encrypted before it reaches storage, which needs a vault, and a vault needs
/// either making or opening first. Neither is a failure to report — both are a
/// thing to do next, so they are states of the editor rather than refusals.
public enum SecretGate: Equatable, Sendable {
    /// There is no vault on this device yet. The first secret makes one.
    case setup
    /// There is a vault, and it is shut.
    case shut
}

/// The body of a template, split into the words and the blanks.
///
/// The split is done on the shared engine's own preview output — it marks each
/// blank with `‹name›` — so the rule for what counts as a blank stays in one
/// place. Nothing here parses template syntax.
public struct TemplateBody: Equatable, Sendable {
    public enum Piece: Equatable, Sendable {
        case text(String)
        case blank(String)
    }

    public let pieces: [Piece]

    public var blanks: [String] {
        pieces.compactMap {
            if case let .blank(name) = $0 { return name }
            return nil
        }
    }

    /// - Parameter preview: the engine's preview, where every blank has been
    ///   replaced by `‹name›`.
    public init(preview: String) {
        var pieces: [Piece] = []
        var text = ""
        var name: String?
        for character in preview {
            switch character {
            case "‹":
                if !text.isEmpty {
                    pieces.append(.text(text))
                    text = ""
                }
                name = ""
            case "›":
                if let open = name {
                    pieces.append(.blank(open))
                    name = nil
                } else {
                    text.append(character)
                }
            default:
                if name != nil {
                    name?.append(character)
                } else {
                    text.append(character)
                }
            }
        }
        // An unclosed marker is text, not a blank: a half-written placeholder
        // must read as what it literally is rather than becoming a field. It
        // rejoins the run it interrupted, so the sentence stays one piece
        // rather than splitting at a character that turned out to mean nothing.
        if let unclosed = name {
            text = "‹" + unclosed + text
            if case let .text(preceding) = pieces.last {
                pieces.removeLast()
                text = preceding + text
            }
        }
        if !text.isEmpty { pieces.append(.text(text)) }
        self.pieces = pieces
    }
}

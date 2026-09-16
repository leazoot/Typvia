// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// A slip of paper rising from the bottom of whatever app the reader is in.
///
/// By the time it arrives the text is already set: a title, a trigger and a
/// kind have all been guessed, and saving without touching any of them is a
/// complete action.

/// Where the sheet is.
public enum ShareStep: Equatable, Sendable {
    case composing
    /// Saved. The page does not celebrate; it teaches one sentence and gets
    /// out of the way.
    case saved(trigger: String, position: UInt32)
}

/// What the sheet guessed, before the reader touches anything.
///
/// The guesses are made from the text itself, on this device. Nothing is sent
/// anywhere to work out what kind of thing was shared.
public struct SharedDraft: Equatable, Sendable {
    public var title: String
    public var body: String
    public var trigger: String
    public var sort: TypeSort

    public init(title: String, body: String, trigger: String, sort: TypeSort) {
        self.title = title
        self.body = body
        self.trigger = trigger
        self.sort = sort
    }

    /// Titles are the first line, trimmed to something that fits a row. A
    /// shared paragraph's first line is almost always what a person would
    /// have called it.
    public static func title(from body: String) -> String {
        let firstLine = body
            .split(separator: "\n", omittingEmptySubsequences: true)
            .first
            .map(String.init) ?? ""
        return String(firstLine.trimmingCharacters(in: .whitespaces).prefix(titleLimit))
    }

    static let titleLimit = 24

    /// The kinds the strip offers first. The rest are a swipe away — four is
    /// what fits without the strip becoming a second keyboard.
    public static let offered: [TypeSort] = [.text, .code, .command, .prompt]
}

/// A trigger that is already taken gets a digit rather than an interruption.
///
/// The rule that decides whether it collides is the core's; what this does is
/// pick the next free spelling so the reader is not stopped mid-save to think
/// of one.
public enum TriggerSuggestion {
    public static func next(after trigger: String, taken: Set<String>) -> String {
        guard taken.contains(trigger) else { return trigger }
        var index = 2
        while taken.contains("\(trigger)\(index)") {
            index += 1
        }
        return "\(trigger)\(index)"
    }
}

/// One row of a widget.
///
/// Widgets show a title and a mark. They never show a body — not a preview,
/// not a truncation, not for a normal snippet and certainly not for a secret,
/// because a widget sits on a screen other people can see.
public struct WidgetRow: Identifiable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let sort: TypeSort?
    /// A secret says it needs verifying, in the place a trigger would be.
    public let isSecret: Bool
    public let trigger: String?

    public init(entry: SnapshotEntry) {
        id = entry.id
        title = entry.title
        sort = entry.isSensitive ? .secret : TypeSort(coreType: entry.snippetType)
        isSecret = entry.isSensitive
        // A secret's trigger is withheld along with everything else: a trigger
        // is a thing you can type at a keyboard to get the value out.
        trigger = entry.isSensitive ? nil : entry.trigger
    }
}

/// How much a widget of each size shows.
public enum WidgetSize: Sendable {
    /// One snippet, tapped to copy — or, in the other small variant, nothing
    /// but a way in.
    case small
    case medium
    /// The lock screen gets a count and nothing else. It is the most public
    /// screen a phone has.
    case lock

    public var rowLimit: Int {
        switch self {
        case .small: 1
        case .medium: 3
        case .lock: 0
        }
    }

    public var showsRows: Bool { rowLimit > 0 }
}

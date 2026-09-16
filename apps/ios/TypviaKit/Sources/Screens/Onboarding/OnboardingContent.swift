// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// Two devices' carets finding each other, and the three screens a reader sees
/// the first time.

/// Where a pairing is up to.
public enum PairingStep: Equatable, Sendable {
    /// Showing this device's code for the other one to read.
    ///
    /// - Parameter expiresAt: when the session stops being honoured, if a
    ///   server said. Nil when nobody promised a window — a WebDAV folder has
    ///   no session server, and a screen that counted one down anyway would be
    ///   stating an expiry no one undertook.
    case offering(code: String, expiresAt: Int64?)
    /// Both devices have arrived at the same check string. If the two agree,
    /// nothing was in the middle.
    case comparing(sas: String, otherDevice: String)
    case joined
    /// The reader said the two sides did not match, or the round failed. The
    /// existing snippets are untouched either way.
    case dropped

    /// Which of the two numbered steps this is.
    public var number: Int {
        switch self {
        case .offering, .dropped: 1
        case .comparing, .joined: 2
        }
    }

    public static let total = 2
}

/// The code on screen, and how long it is good for.
///
/// - Parameter secondsLeft: nil when **nobody promised a window** — a WebDAV
///   folder has no session server, and a screen counting one down anyway would
///   be stating an expiry no one undertook. Folded to zero, it declared a
///   just-born code dead.
public struct PairingCode: Equatable, Sendable {
    public let code: String
    public let secondsLeft: TimeInterval?

    public init(code: String, secondsLeft: TimeInterval?) {
        self.code = code
        self.secondsLeft = secondsLeft
    }

    /// Codes stop being valid; they do not quietly refresh themselves behind
    /// the reader's back, because a code that changed while they were reading
    /// it out is a code that will not work. A code nobody promised a window for
    /// has not expired — nothing was undertaken to run out.
    public var hasExpired: Bool { secondsLeft.map { $0 <= 0 } ?? false }

    /// Rounded up, in minutes rather than in seconds-then-divided: a code
    /// with forty seconds left is good for a minute, and telling a reader
    /// "0 minutes" while it still works is telling them it is dead. Zero when
    /// there is no window — the screen states nothing at all in that case.
    public var minutesLeft: Int {
        guard let secondsLeft else { return 0 }
        return max(Int((secondsLeft / 60).rounded(.up)), 0)
    }

    /// Grouped for reading aloud. A wall of characters is read wrong once and
    /// then blamed on the product.
    public static func grouped(_ code: String, every size: Int = 4) -> String {
        guard size > 0 else { return code }
        return stride(from: 0, to: code.count, by: size).map { offset in
            let start = code.index(code.startIndex, offsetBy: offset)
            let end = code.index(start, offsetBy: min(size, code.count - offset))
            return String(code[start..<end])
        }
        .joined(separator: " ")
    }
}

/// One group of the check string, and how much of it has landed.
///
/// A group is never split across lines: the reader is comparing it against
/// another screen, and half a group on each line is how a comparison is read
/// wrong.
public struct SasGroup: Equatable, Sendable, Identifiable {
    public let id: Int
    public let characters: [Character]
    public let shown: Int

    init(id: Int, characters: [Character], shown: Int) {
        self.id = id
        self.characters = characters
        self.shown = min(max(shown, 0), characters.count)
    }

    /// The group as it is set, characters that have not landed yet holding
    /// their place. The face is mono, so a blank is exactly as wide as a letter
    /// and nothing shifts sideways as the rest arrive.
    public var text: String {
        String(characters.enumerated().map { $0.offset < shown ? $0.element : " " })
    }

    /// Only what has landed. This screen never speaks a character it has not
    /// shown.
    var landed: String { String(characters.prefix(shown)) }
}

/// The check string, revealed one character at a time.
///
/// Each half is computed by one of the two devices, so the pair agreeing is
/// the proof that nothing sat in the middle. They arrive in sequence because a
/// reader has to read them out — and because arriving at once invites a glance
/// instead of a comparison.
///
/// How long the string is belongs to the core, not to this screen: it hands
/// over groups joined by hyphens and this lays out however many it gets. The
/// hyphens separate rather than say anything, so they are dropped here and the
/// gap between groups carries them. Laying the groups out in rows is the whole
/// point — a string set on one line runs off the edge of the phone, and
/// characters the reader cannot see are characters they cannot compare.
public struct SasReveal: Equatable, Sendable {
    /// What the reader compares: the characters, separators dropped.
    public let characters: [Character]
    public let shown: Int
    /// The groups, in the order the core hyphenated them.
    public let groups: [SasGroup]

    public init(sas: String, shown: Int) {
        characters = Array(sas.filter { $0 != SasReveal.groupSeparator })
        let landed = min(max(shown, 0), characters.count)
        self.shown = landed
        groups = SasReveal.groups(of: sas, shown: landed)
    }

    /// The groups packed into rows no wider than `perRow` characters.
    ///
    /// How many fit is the screen's question, not the string's, which is why
    /// the caller passes it: type at an accessibility step is half again as
    /// wide, and a row that fitted at the standard steps stops fitting.
    public func rows(perRow: Int = SasReveal.charactersPerRow) -> [[SasGroup]] {
        var rows: [[SasGroup]] = []
        var width = 0
        for group in groups {
            if rows.isEmpty || width + group.characters.count > perRow {
                rows.append([group])
                width = group.characters.count
            } else {
                rows[rows.count - 1].append(group)
                width += group.characters.count
            }
        }
        return rows
    }

    /// How far the line under the characters has grown.
    public var progress: Double {
        guard !characters.isEmpty else { return 0 }
        return Double(shown) / Double(characters.count)
    }

    public var isComplete: Bool { shown == characters.count && !characters.isEmpty }

    /// What a reader who cannot see the screen is told, groups separated so the
    /// reading has somewhere to breathe.
    public var spoken: String {
        groups.map(\.landed).filter { !$0.isEmpty }.joined(separator: " ")
    }

    /// The step interval that spreads the reveal across the delivery's window.
    public static func interval(for count: Int, over window: Double = 0.720) -> Double {
        guard count > 0 else { return window }
        return window / Double(count)
    }

    /// How many characters a row carries at the standard Dynamic Type steps.
    /// Ten of them, set at the code rung, fit the narrowest phone this app runs
    /// on — the check has to be makeable there too, not only on a roomy screen.
    public static let charactersPerRow = 10

    /// And at an accessibility step, where the same ten no longer fit: one
    /// group to a line. Taller, and still the whole string — dropping half of
    /// it is what made this check unmakeable in the first place.
    public static let charactersPerStackedRow = 5

    /// How the core joins the groups it hands over.
    private static let groupSeparator: Character = "-"

    private static func groups(of sas: String, shown: Int) -> [SasGroup] {
        var landed = 0
        let parts: [Substring] = sas.split(separator: groupSeparator)
        return parts.enumerated().map { index, text in
            let group = SasGroup(id: index, characters: Array(text), shown: shown - landed)
            landed += text.count
            return group
        }
    }

}

/// The three screens of a first launch.
///
/// No illustrations and no carousel: what is demonstrated is the product
/// itself — a line being typed out with the trigger that would have produced
/// it. Every screen can be skipped, and the way out is never dimmed into a
/// trap.
public enum FirstRun: Int, CaseIterable, Identifiable, Sendable {
    case save = 1
    case call
    case keyboard

    public var id: Int { rawValue }

    public static let total = FirstRun.allCases.count

    /// The line the screen types out, and the trigger under it. These are the
    /// delivery's own examples.
    public var demonstration: (body: String, trigger: String)? {
        switch self {
        case .save: ("./deploy.sh --env=prod --yes", ";deploy")
        case .call: ("kubectl rollout status deploy/api", ";k8s")
        case .keyboard: nil
        }
    }
}

/// Whether the keyboard has been added in the system settings.
///
/// It is checked rather than assumed: the reader leaves the app to do it, and
/// the app has to know whether they actually did when they come back.
/// What the keyboard step of the first run says.
///
/// Out of the view for the same reason as the panel's: these are the sentences
/// that told the reader to turn on a switch this product declares it does not
/// want, and a third step nobody could carry out.
public enum FirstRunKeyboardCopy {
    public static func intro(_ tr: Translator) -> String {
        tr(
            "Settings → Keyboard → Typvia. It never asks for full access: it reads this device only, never the network, and never what you type.",
            "设置 → 键盘 → Typvia。它不申请「完全访问权限」:只读本机,不联网,也不记录你打的字。"
        )
    }

    /// Two steps, not three. The third used to be "turn on full access".
    public static func steps(_ tr: Translator) -> [String] {
        [
            tr("Open the system Settings", "打开系统设置"),
            tr("Keyboard → add Typvia", "键盘 → 添加 Typvia"),
        ]
    }
}

public enum KeyboardInstall: Equatable, Sendable {
    case notAdded
    case ready

    /// The keyboard's bundle identifier, as it appears in the system's list of
    /// active input modes.
    public static let bundleId = "dev.typvia.mobile.keyboard"

    public static func state(activeInputModes: [String]) -> KeyboardInstall {
        activeInputModes.contains(bundleId) ? .ready : .notAdded
    }
}

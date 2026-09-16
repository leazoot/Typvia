// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// Every word the vault chapter says, in one place.
///
/// Each of these is about this device's own vault and each one is checkable.
/// None of them is a reassurance: "your data is encrypted" is a sentence that
/// appears on products whose data is not.
enum VaultChapterCopy {
    /// The chapter's two facts on one line: how it opens, and when it shuts.
    ///
    /// These used to be two labelled blocks with a paragraph under each. The
    /// paragraphs are gone: a reader scanning this page wants the two values,
    /// and the reasoning behind them belongs in the repository's documents,
    /// not on the page somebody opened to check a setting.
    static func standing(_ facts: VaultFacts, _ tr: Translator) -> String {
        guard let window = idleWindow(facts, tr) else {
            return tr("Master password", "主密码解锁")
        }
        return tr(
            "Master password · shuts after \(window)",
            "主密码解锁 · \(window)后回锁"
        )
    }

    /// The idle window, in whole minutes.
    ///
    /// Rounded down and never to zero: a window of forty seconds printed as
    /// "0 minutes" reads as "it does not re-lock", which is the opposite of
    /// true. Absent where there is no vault, and where the window is not a
    /// window — there is nothing to state in either case.
    static func idleWindow(_ facts: VaultFacts, _ tr: Translator) -> String? {
        guard facts.isMade, facts.idleTimeoutMs > 0 else { return nil }
        let minutes = facts.idleTimeoutMs / 60_000
        guard minutes >= 1 else { return tr("under a minute", "不到一分钟") }
        return tr.counted(minutes, "minute", "minutes", "\(minutes) 分钟")
    }

    /// The contents page's right-hand column: the shortest true thing about
    /// the chapter, in the same figure the page itself states.
    static func contentsValue(_ facts: VaultFacts, _ tr: Translator) -> String {
        guard facts.isMade else { return tr("not made yet", "尚未建立") }
        guard facts.idleTimeoutMs > 0 else { return tr("made", "已建立") }
        let minutes = facts.idleTimeoutMs / 60_000
        guard minutes >= 1 else { return tr("shuts in under a min", "不到一分钟回锁") }
        return tr("shuts after \(minutes) min", "\(minutes) 分钟回锁")
    }

    static func headline(_ facts: VaultFacts, _ tr: Translator) -> String {
        facts.isMade
            ? tr(
                "It is shut right now, and it shuts itself whenever you stop using it.",
                "它此刻锁着,而且你一停下不用,它就会自己锁上。"
            )
            : tr(
                "There is no vault on this device yet. Saving a password or a key makes one.",
                "这台设备上还没有保险库。存下一条密码或密钥就会建起来。"
            )
    }

    /// What a refusal means on this page.
    ///
    /// Written here rather than taken from the shared sentence because this
    /// page knows what the reader was doing. Two of these say what is still
    /// true first: a failed act leaves the vault exactly as it was, and a page
    /// that only reports the failure leaves a reader guessing at the state.
    static func refusal(_ refusal: Refusal, _ tr: Translator) -> String {
        switch refusal {
        // The core folds a wrong password in with too many attempts in a row,
        // so this says what did not happen rather than which of the two it
        // was — naming one of them would be a guess printed as a fact.
        case .notPermitted:
            return tr("It did not open with that. Nothing changed.", "用这个没打开。什么都没变。")
        case .clash:
            return tr(
                "There is no vault here to keep a key for.",
                "这里还没有保险库,谈不上给它留钥匙。"
            )
        case .storage:
            return tr(
                "The key store would not take that copy. The vault is as it was.",
                "钥匙存放处不肯收这份副本。保险库还是原来的样子。"
            )
        default:
            return refusal.sentence(tr)
        }
    }
}

/// The vault chapter: how it opens, when it shuts, and what is true of what is
/// inside.
///
/// It is set on ordinary paper rather than in the ink room. This is a page
/// *about* the vault; the material break belongs to the vault itself, and a
/// settings page wearing it would be borrowing weight it has not earned.
struct VaultChapter: View {
    @Environment(\.tr) private var tr
    @State private var password = ""
    @State private var isEnrolling = false

    let facts: VaultFacts
    /// What the last act did, and why it did not. Never both.
    var said: String?
    var refusal: Refusal?
    /// Absent while nothing can be changed — then the page is what it mostly
    /// is anyway: a statement of how this device's vault behaves.
    var keep: ((String) -> Void)?
    var remove: (() -> Void)?
    var forget: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(VaultChapterCopy.headline(facts, tr))
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)
            if facts.isMade, keep != nil, remove != nil {
                faceId
            }
            // The two facts a reader scans for, and nothing else.
            Text(VaultChapterCopy.standing(facts, tr))
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, SettingsMetrics.rowGap)
        }
        // A report belongs to the act that produced it. Leaving the page ends
        // both, so re-opening the chapter does not congratulate somebody on
        // something they did yesterday.
        .onDisappear {
            forget?()
            password = ""
            isEnrolling = false
        }
    }

    /// The Face ID copy, as two acts rather than as a state.
    ///
    /// Which one is in force cannot be read: asking the store whether the copy
    /// is there means reading it, and reading it *is* the Face ID sheet. So
    /// both are offered, and the page says so instead of guessing.
    private var faceId: some View {
        VStack(alignment: .leading, spacing: 0) {
            PaperLabel(tr("FACE ID", "面容 ID"))
                .padding(.top, SettingsMetrics.lineGap)
            if isEnrolling {
                PaperField(
                    label: tr("MASTER PASSWORD", "主密码"),
                    text: $password,
                    isSecret: true
                )
                StickVerb(title: tr("Keep it", "留下"), isPrimary: true) {
                    let typed = password
                    password = ""
                    isEnrolling = false
                    keep?(typed)
                }
                .padding(.top, SettingsMetrics.rowGap)
            } else {
                // The one a reader came here to do carries the accent; its
                // undo stands beside it as a word.
                StickVerb(title: tr("Keep a copy for Face ID", "给面容 ID 留一份"), isPrimary: true) {
                    forget?()
                    isEnrolling = true
                }
                .padding(.top, SettingsMetrics.rowGap)
            }
            StickVerb(title: tr("Remove that copy", "删掉那份副本")) { remove?() }
            .padding(.top, SettingsMetrics.lineInnerGap)
            if let said {
                report(said, colour: Paper.ink2)
            }
            if let refusal {
                report(VaultChapterCopy.refusal(refusal, tr), colour: Paper.attention)
            }
        }
    }

    private func aside(_ sentence: String) -> some View {
        Text(sentence)
            .typviaType(.caption)
            .foregroundStyle(Paper.ink2)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, SettingsMetrics.lineInnerGap)
    }

    private func report(_ sentence: String, colour: Color) -> some View {
        Text(sentence)
            .typviaType(.caption)
            .foregroundStyle(colour)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, SettingsMetrics.rowGap)
    }
}

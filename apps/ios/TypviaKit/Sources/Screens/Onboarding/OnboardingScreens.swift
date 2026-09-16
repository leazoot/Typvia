// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI
import UIKit

/// This device's pairing code, held out for the other one to read.
///
/// The code is the page: set large, on a slip of paper, like a sticker. How
/// long it lasts is one mono line rather than a countdown ring, and the way
/// round it — a code you can read aloud — is always on screen instead of
/// hidden behind "more".
struct PairingOffer: View {
    @Environment(\.tr) private var tr

    let code: PairingCode
    let useTextCode: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            StepCount(step: 1)
            Text(tr("Hand this one to the other device", "把这台交给另一台"))
                .typviaType(.title2)
                .foregroundStyle(Paper.ink)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, OnboardingMetrics.headlineGap)
            Text(
                tr(
                    "Open Typvia on the other device and point it at this code. The key passes between the two of them and never goes through the server.",
                    "在另一台设备上打开 Typvia,对准这个码。钥匙在两台设备之间直接交换,不经过服务器。"
                )
            )
            .typviaType(.bodyS)
            .foregroundStyle(Paper.ink2)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, OnboardingMetrics.bodyGap)
            PaperCard {
                Text(PairingCode.grouped(code.code))
                    .typviaType(.mono)
                    .foregroundStyle(Paper.ink)
                    .padding(OnboardingMetrics.codePadding)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .opacity(code.hasExpired ? OnboardingMetrics.expiredOpacity : 1)
            .padding(.top, OnboardingMetrics.codeGap)
            if let validity {
                Text(validity)
                    .typviaType(.mono)
                    .foregroundStyle(Paper.ink3)
                    .padding(.top, OnboardingMetrics.bodyGap)
            }
            if let useTextCode {
                StickVerb(title: tr("No camera? Use the text code", "没有摄像头?改用文字码"), action: useTextCode)
                    .padding(.top, OnboardingMetrics.actionGap)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// Expiry is stated, not animated. An expired code says so and waits to be
    /// asked for a new one rather than swapping itself out mid-sentence.
    ///
    /// Nil when nobody promised a window: a WebDAV folder has no session
    /// server, and the line is left off rather than filled in with an expiry
    /// no one undertook.
    private var validity: String? {
        guard code.secondsLeft != nil else { return nil }
        if code.hasExpired {
            return tr("This code has expired — ask for a new one", "这个码过期了 · 重新出码")
        }
        return code.minutesLeft == 1
            ? tr("Good for 1 more minute", "1 分钟内有效")
            : tr("Good for \(code.minutesLeft) more minutes", "\(code.minutesLeft) 分钟内有效")
    }
}

/// The check string both devices arrived at.
///
/// It is the memorable moment of the flow: set large, landing one character at
/// a time with a line growing underneath. If the two sides read the same, there
/// was nothing in the middle.
struct SasCheck: View {
    @Environment(\.tr) private var tr
    @Environment(\.dynamicTypeSize) private var typeSize

    let reveal: SasReveal
    /// The line under the characters: who is waiting, or — on the joining side,
    /// which holds no name for the other device — the account's fingerprint.
    /// The sentence is the caller's, because only the caller knows which of
    /// the two sides it is on.
    let note: String
    let confirm: (() -> Void)?
    let reject: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            StepCount(step: 2)
            Text(tr("Is it the same on both?", "两边是同一串吗?"))
                .typviaType(.title2)
                .foregroundStyle(Paper.ink)
                .padding(.top, OnboardingMetrics.headlineGap)
            Text(
                tr(
                    "Each of these characters was worked out half by one device and half by the other. If both sides read the same, nothing sat in between.",
                    "这串字符由两台设备各算一半得出。只要两边一样,中间就没有第三者。"
                )
            )
            .typviaType(.bodyS)
            .foregroundStyle(Paper.ink2)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, OnboardingMetrics.bodyGap)
            characters
                .padding(.top, OnboardingMetrics.codeGap)
            Text(note)
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
                .padding(.top, OnboardingMetrics.bodyGap)
            actions
                .padding(.top, OnboardingMetrics.actionGap)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// At an accessibility step the same row is half again as wide, so it
    /// carries one group instead of two. The string is never what gives way.
    private var charactersPerRow: Int {
        Reflow(for: typeSize) == .stacked
            ? SasReveal.charactersPerStackedRow
            : SasReveal.charactersPerRow
    }

    private var characters: some View {
        VStack(alignment: .leading, spacing: OnboardingMetrics.sasLineGap) {
            ForEach(Array(reveal.rows(perRow: charactersPerRow).enumerated()), id: \.offset) { _, row in
                HStack(spacing: OnboardingMetrics.sasGroupGap) {
                    ForEach(row) { group in
                        Text(group.text)
                            .typviaType(.code)
                            .foregroundStyle(Paper.ink)
                    }
                }
            }
            Rectangle()
                .fill(Room.vault.accent)
                .frame(height: OnboardingMetrics.sasRuleHeight)
                .scaleEffect(x: reveal.progress, anchor: .leading)
                .accessibilityHidden(true)
        }
        .accessibilityElement(children: .combine)
        // One thing to read rather than twenty: read out character by
        // character, the check is a stream of letters with no grouping.
        .accessibilityLabel(reveal.spoken)
    }

    @ViewBuilder
    private var actions: some View {
        VStack(alignment: .leading, spacing: OnboardingMetrics.bodyGap) {
            HStack(spacing: OnboardingMetrics.actionGap) {
                if let confirm {
                    StickVerb(title: tr("Same — carry on", "一样,继续"), isPrimary: true, action: confirm)
                        .disabled(!reveal.isComplete)
                        .opacity(reveal.isComplete ? 1 : OnboardingMetrics.expiredOpacity)
                }
                if let reject {
                    StickVerb(title: tr("Not the same", "不一样"), action: reject)
                }
            }
            Text(
                tr(
                    "Choosing \"not the same\" drops the connection and asks for a new code. The snippets you already have are untouched.",
                    "选「不一样」会立刻断开并重出码,已有的片段不受影响。"
                )
            )
            .typviaType(.caption)
            .foregroundStyle(Paper.ink3)
            .fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// One of the three first-launch screens.
///
/// The demonstration is the product: a line types itself out with the trigger
/// that would have produced it underneath. No illustrations, and the way
/// straight in is never dimmed.
struct FirstRunScreen: View {
    @Environment(\.tr) private var tr

    let page: FirstRun
    let next: (() -> Void)?
    let skip: (() -> Void)?
    let openSystemSettings: (() -> Void)?
    let install: KeyboardInstall

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            StepBars(current: page.rawValue, total: FirstRun.total)
            Text(headline)
                .typviaType(.title2)
                .foregroundStyle(Paper.ink)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, OnboardingMetrics.headlineGap)
            if let demonstration = page.demonstration {
                VStack(alignment: .leading, spacing: OnboardingMetrics.bodyGap) {
                    TypedLine(text: demonstration.body)
                    Text(demonstration.trigger)
                        .typviaType(.mono)
                        .foregroundStyle(Room.home.accent)
                }
                .padding(.top, OnboardingMetrics.codeGap)
                Text(tr("Save once; three characters do the rest.", "存一次,以后三个字符就够"))
                    .typviaType(.caption)
                    .foregroundStyle(Paper.ink3)
                    .padding(.top, OnboardingMetrics.bodyGap)
            } else {
                keyboardSteps
            }
            actions
                .padding(.top, OnboardingMetrics.actionGap)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var headline: String {
        switch page {
        case .save:
            return tr(
                "The line you have already typed for the third time today.",
                "你今天已经打过第三遍的那句话。"
            )
        case .call:
            return tr(
                "Two characters bring it back, in any app.",
                "两个字符就能把它叫回来,在任何 app 里。"
            )
        case .keyboard:
            return tr(
                "The keyboard is added once, so it can appear in other apps.",
                "键盘要装一次,才能在别的 app 里出现。"
            )
        }
    }

    /// The step people drop out at, so the steps are written down rather than
    /// gestured at.
    ///
    /// This used to explain what full access was for and then ask for it as a
    /// third step. This keyboard declares that it does not want full access,
    /// so that switch is not drawn in the system's settings at all — the step
    /// was one nobody could carry out, and the paragraph was reassurance about
    /// a permission that is never requested.
    private var keyboardSteps: some View {
        VStack(alignment: .leading, spacing: OnboardingMetrics.bodyGap) {
            Text(FirstRunKeyboardCopy.intro(tr))
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)
            ForEach(Array(steps.enumerated()), id: \.offset) { index, step in
                HStack(alignment: .top, spacing: OnboardingMetrics.stepGap) {
                    Text(tr("Step \(index + 1)", "第 \(index + 1) 步"))
                        .typviaType(.monoLabel)
                        .foregroundStyle(Paper.ink3)
                        .frame(width: OnboardingMetrics.stepLabelWidth, alignment: .leading)
                    Text(step)
                        .typviaType(.bodyS)
                        .foregroundStyle(Paper.ink)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            if install == .ready {
                Text(tr("The keyboard is ready", "键盘已就绪"))
                    .typviaType(.mono)
                    .foregroundStyle(Room.vault.accent)
            }
        }
        .padding(.top, OnboardingMetrics.codeGap)
    }

    private var steps: [String] { FirstRunKeyboardCopy.steps(tr) }

    /// A page whose work is done offers the way onward, not the way to do it
    /// again.
    ///
    /// The keyboard page used to send the reader to the system settings and
    /// offer "Later" whichever state it was in — so a keyboard that was
    /// already added still read as an unfinished errand, directly under a line
    /// saying it was ready. Being done is the one state where a step's own
    /// verb is the wrong one to print.
    private var isErrandDone: Bool { page == .keyboard && install == .ready }

    /// The last page's way onward is the way in, so it says so rather than
    /// promising a next page there is none of.
    private var onwardTitle: String {
        page == .keyboard ? tr("Start using it", "开始用") : tr("Next", "下一句")
    }

    @ViewBuilder
    private var actions: some View {
        HStack(spacing: OnboardingMetrics.actionGap) {
            if page == .keyboard, !isErrandDone, let openSystemSettings {
                StickVerb(title: tr("Open Settings", "去系统设置"), isPrimary: true, action: openSystemSettings)
            } else if let next {
                StickVerb(title: onwardTitle, isPrimary: true, action: next)
            }
            // Never dimmed into a trap: the way straight in is as plain as the
            // way onward. Once the errand is done there is nothing left to
            // defer, so the second word goes rather than becoming a puzzle
            // printed under a line saying the keyboard is ready.
            if let skip, !isErrandDone {
                StickVerb(
                    title: page == .keyboard
                        ? tr("Later", "以后再说")
                        : tr("Just start using it", "直接开始用"),
                    action: skip
                )
            }
        }
    }
}

/// A line of the product, typing itself out and then holding.
///
/// The first run demonstrates the thing rather than illustrating it, so this
/// is the demonstration: the same characters a reader would have typed,
/// arriving at typing speed and then staying long enough to be read.
///
/// It starts complete and is emptied by the loop, not the other way round —
/// anything that draws this without running (a still, a rendered frame, a
/// reader who has motion turned off) then gets the whole line rather than a
/// blank space where the point was.
struct TypedLine: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let text: String

    @State private var shown: Int?

    var body: some View {
        Text(String(text.prefix(shown ?? text.count)))
            .typviaType(.mono)
            .foregroundStyle(Paper.ink)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            // The whole line is what is being said; hearing it grow character
            // by character would be nonsense.
            .accessibilityLabel(text)
            .task(id: text) {
                guard !reduceMotion else { return }
                await type()
            }
    }

    private func type() async {
        let step = UInt64(OnboardingMetrics.typeWindow / Double(max(text.count, 1)) * 1_000_000_000)
        while !Task.isCancelled {
            shown = 0
            for count in 1...max(text.count, 1) {
                try? await Task.sleep(nanoseconds: step)
                guard !Task.isCancelled else { return }
                shown = count
            }
            try? await Task.sleep(nanoseconds: UInt64(OnboardingMetrics.typeHold * 1_000_000_000))
        }
    }
}

/// "1 / 2" — where the reader is, in mono.
struct StepCount: View {
    let step: Int

    var body: some View {
        Text("\(step) / \(PairingStep.total)")
            .typviaType(.mono)
            .foregroundStyle(Paper.ink3)
    }
}

/// Three short rules rather than dots: the first-launch progress is a measure,
/// not a carousel.
struct StepBars: View {
    let current: Int
    let total: Int

    var body: some View {
        HStack(spacing: OnboardingMetrics.barGap) {
            ForEach(1...total, id: \.self) { index in
                Rectangle()
                    .fill(Paper.ink)
                    .opacity(index == current ? 1 : OnboardingMetrics.barDim)
                    .frame(width: OnboardingMetrics.barWidth, height: OnboardingMetrics.barHeight)
            }
        }
        .accessibilityElement()
        .accessibilityLabel("\(current) / \(total)")
    }
}

enum OnboardingMetrics {
    static let headlineGap: CGFloat = 18
    static let bodyGap: CGFloat = 12
    static let codeGap: CGFloat = 30
    static let codePadding: CGFloat = 20
    static let actionGap: CGFloat = 26
    static let expiredOpacity = 0.2
    /// Between groups, not between characters: the face is mono and sets its
    /// own even rhythm inside a group.
    static let sasGroupGap: CGFloat = 14
    static let sasLineGap: CGFloat = 14
    static let sasRuleHeight: CGFloat = 2
    static let stepGap: CGFloat = 14
    static let stepLabelWidth: CGFloat = 62
    static let barWidth: CGFloat = 22
    static let barHeight: CGFloat = 2
    static let barGap: CGFloat = 6
    static let barDim = 0.2
    /// The delivery's demonstration loop: typed out, then held long enough to
    /// read before it starts over.
    static let typeWindow = 1.800
    static let typeHold = 0.600
}

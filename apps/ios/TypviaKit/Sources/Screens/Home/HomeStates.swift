// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// The empty sheet.
///
/// A caret breathing on its own, one sentence with a person in it, and a way
/// in. No illustration, no "no data", no shrug.
struct EmptySheet: View {
    @Environment(\.tr) private var tr
    @Environment(\.dynamicTypeSize) private var typeSize

    let actions: HomeActions

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Caret(height: HomeMetrics.emptyCaretHeight, capped: true)
                .padding(.horizontal, HomeMetrics.emptyCaretOverhangRoom)
                .padding(.bottom, Tokens.Space.group)
            Text(tr("This is still a blank sheet.", "这里还是一张白纸。"))
                .typviaType(.title2)
                .foregroundStyle(Paper.ink)
            Text(
                tr(
                    "Save the sentence you have already typed twice today as your first snippet.",
                    "把你今天已经重复输入过两次的那句话,存成第一枚片段。"
                )
            )
            .typviaType(.body)
            .foregroundStyle(Paper.ink2)
            .padding(.top, HomeMetrics.emptyBodyGap)
            buttons
            Text(
                tr(
                    "Or select text in any app →\nshare it to Typvia",
                    "或在任意 app 里选中文字 →\n分享给 Typvia"
                )
            )
            .typviaType(.mono)
            .foregroundStyle(Paper.ink3)
            .padding(.top, HomeMetrics.emptyAsideGap)
        }
        // Sentences get the height they need. The delivery's rule for stepped
        // -up type is that lines are never dropped to make things fit, and a
        // stack that hands out heights by proposal will drop them.
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, HomeMetrics.emptyInset)
    }

    /// Stepped-up type stacks the buttons rather than shrinking or wrapping
    /// them — the delivery's instruction for this screen by name.
    @ViewBuilder
    private var buttons: some View {
        let items = buttonItems
        if !items.isEmpty {
            let layout = Reflow(for: typeSize) == .stacked
                ? AnyLayout(VStackLayout(alignment: .leading, spacing: HomeMetrics.buttonGap))
                : AnyLayout(HStackLayout(spacing: HomeMetrics.buttonGap))
            layout {
                ForEach(items) { item in
                    PressButton(title: item.title, isPrimary: item.isPrimary, action: item.action)
                }
            }
            .padding(.top, HomeMetrics.emptyButtonsGap)
        }
    }

    /// Only the ways in that exist get drawn. A screen that offers to save the
    /// reader's first snippet and then does nothing when tapped teaches them
    /// not to trust the next button either.
    private var buttonItems: [EmptyAction] {
        var items: [EmptyAction] = []
        if let compose = actions.compose {
            items.append(
                EmptyAction(
                    title: tr("Save your first one", "存下第一枚"),
                    isPrimary: true,
                    action: compose
                )
            )
        }
        if let importClipboard = actions.importClipboard {
            items.append(
                EmptyAction(
                    title: tr("Import from clipboard", "从剪贴板导入"),
                    isPrimary: false,
                    action: importClipboard
                )
            )
        }
        return items
    }

    private struct EmptyAction: Identifiable {
        let title: String
        let isPrimary: Bool
        let action: () -> Void
        var id: String { title }
    }
}

/// A button that presses into the paper: it sinks, briefly, and nothing else
/// moves.
struct PressButton: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isPressed = false

    let title: String
    let isPrimary: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .typviaType(.body)
                .foregroundStyle(isPrimary ? Paper.base : Paper.ink2)
                .lineLimit(1)
                .padding(.horizontal, HomeMetrics.buttonPaddingX)
                .padding(.vertical, HomeMetrics.buttonPaddingY)
                .frame(minHeight: Tokens.Hit.minimum)
                .background(background)
        }
        .buttonStyle(.plain)
        .scaleEffect(isPressed && !reduceMotion ? HomeMetrics.buttonPressScale : 1)
        .animation(Beat.tap.enter(reduceMotion: reduceMotion), value: isPressed)
        .simultaneousGesture(
            DragGesture(minimumDistance: 0)
                .onChanged { _ in
                    guard !isPressed else { return }
                    isPressed = true
                    Haptic.light.play()
                }
                .onEnded { _ in isPressed = false }
        )
    }

    @ViewBuilder
    private var background: some View {
        let shape = RoundedRectangle(cornerRadius: Tokens.Radius.control, style: .continuous)
        if isPrimary {
            shape.fill(Paper.ink)
        } else {
            shape.strokeBorder(Paper.rule, lineWidth: Tokens.Line.hairlineWidth)
        }
    }
}

/// The skeleton: rows of thin ink where the type will land.
///
/// A compositor sets the lines before the words arrive. There is no spinner
/// anywhere in this product, and this is what stands in its place. Every piece
/// breathes between the same two weights — the frames give the pieces
/// different resting opacities but then animate them all through one pair, so
/// one pair is what this uses.
struct SkeletonShelf: View {
    @Environment(\.tr) private var tr

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            SkeletonBar(width: HomeMetrics.skeletonHeadWidth, delay: 0)
                .frame(height: HomeMetrics.skeletonHeadHeight)
            HStack(spacing: HomeMetrics.cardGap) {
                ForEach(0..<3, id: \.self) { index in
                    SkeletonCard(delay: Double(index) * HomeMetrics.skeletonStagger)
                }
            }
            .padding(.top, HomeMetrics.skeletonGap)
            VStack(spacing: HomeMetrics.skeletonRowGap) {
                ForEach(0..<3, id: \.self) { index in
                    let delay = Double(index) * HomeMetrics.skeletonStagger
                    HStack(spacing: HomeMetrics.triggerGap) {
                        SkeletonBar(width: HomeMetrics.skeletonTriggerWidth, delay: delay)
                        SkeletonBar(width: nil, delay: delay + HomeMetrics.skeletonStagger / 2)
                    }
                    .frame(height: HomeMetrics.skeletonRowHeight)
                }
            }
            .padding(.top, Tokens.Space.group)
        }
        .padding(.trailing, Tokens.Space.screenPadding)
        .accessibilityElement()
        .accessibilityLabel(tr("Reading the local library", "正在读取本地库"))
    }
}

private struct SkeletonCard: View {
    let delay: Double

    var body: some View {
        RoundedRectangle(cornerRadius: Tokens.Radius.card, style: .continuous)
            .fill(Paper.ink)
            .frame(width: HomeMetrics.cardWidth, height: HomeMetrics.cardHeight)
            .modifier(SkeletonBreath(delay: delay))
    }
}

private struct SkeletonBar: View {
    let width: CGFloat?
    let delay: Double

    var body: some View {
        RoundedRectangle(cornerRadius: HomeMetrics.skeletonRadius, style: .continuous)
            .fill(Paper.ink)
            .frame(maxWidth: width ?? .infinity)
            .modifier(SkeletonBreath(delay: delay))
    }
}

/// Ink that has not dried. With reduced motion the rows simply stay at the
/// lighter weight: still legible as a placeholder, not moving.
private struct SkeletonBreath: ViewModifier {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isDark = false

    let delay: Double

    func body(content: Content) -> some View {
        content
            .opacity(isDark ? HomeMetrics.skeletonPeak : HomeMetrics.skeletonFloor)
            .onAppear {
                guard !reduceMotion else { return }
                withAnimation(
                    .easeInOut(duration: HomeMetrics.skeletonBreath / 2)
                        .repeatForever(autoreverses: true)
                        .delay(delay)
                ) {
                    isDark = true
                }
            }
            .accessibilityHidden(true)
    }
}

/// The notice card: the one place on this screen that carries bad news, and
/// the only shape it is allowed to take.
///
/// It says what still works first, in the reader's own numbers. Then what is
/// wrong, limited to what is actually known — never a guess at whose fault it
/// is. Then one thing to do. No red and no warning triangle: a clay tick and
/// words.
struct NoticeCard: View {
    @Environment(\.tr) private var tr

    let notice: HomeNotice
    let total: UInt32
    let isRetrying: Bool
    let retry: () -> Void
    /// Absent when the settings room is not reachable from here; the card then
    /// states the cause and offers nothing, rather than a route to nowhere.
    var openSettings: (() -> Void)?

    var body: some View {
        PaperCard {
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: HomeMetrics.noticeLabelGap) {
                    Rectangle()
                        .fill(Paper.attention)
                        .frame(
                            width: HomeMetrics.noticeTickWidth,
                            height: HomeMetrics.noticeTickHeight
                        )
                    Text(label)
                        .typviaType(.monoLabel)
                        .foregroundStyle(Paper.attention)
                }
                .padding(.bottom, HomeMetrics.noticeLabelBottomGap)
                Text(reassurance)
                    .typviaType(.sectionTitle)
                    .foregroundStyle(Paper.ink)
                Text(problem)
                    .typviaType(.caption)
                    .foregroundStyle(Paper.ink2)
                    .padding(.top, HomeMetrics.noticeBodyGap)
                actions
                    .padding(.top, HomeMetrics.noticeActionsGap)
            }
            .padding(HomeMetrics.noticePadding)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    @ViewBuilder
    private var actions: some View {
        HStack(spacing: HomeMetrics.noticeActionGap) {
            if let act = action {
                Button(action: act.run) {
                    Text(act.title)
                        .typviaType(.bodyS)
                        .foregroundStyle(Paper.attention)
                        .overlay(alignment: .bottom) {
                            Rectangle()
                                .fill(Paper.attention)
                                .frame(height: HomeMetrics.noticeUnderline)
                                .offset(y: HomeMetrics.noticeUnderlineDrop)
                        }
                        .frame(minHeight: Tokens.Hit.minimum)
                }
                .buttonStyle(.plain)
                .disabled(isRetrying)
            }
            if case let .syncPaused(queued, _, _) = notice {
                // A count, not a link: the queue has no screen to open yet.
                Text(tr("\(queued) waiting", "\(queued) 条排队"))
                    .typviaType(.bodyS)
                    .foregroundStyle(Paper.ink2)
            }
        }
    }

    private var label: String {
        switch notice {
        case .syncPaused: tr("SYNC PAUSED", "同步暂停")
        case .loadFailed: tr("COULD NOT READ", "读取失败")
        }
    }

    /// The one thing to do, or nothing.
    ///
    /// Nothing is a real answer here. A round the server told to wait will
    /// resume on its own, and a round refused for a rejected session will be
    /// refused again in exactly the same way — "try again" against either is a
    /// button whose only function is to fail, and the card is the place a
    /// reader is supposed to be able to trust.
    private var action: (title: String, run: () -> Void)? {
        switch notice {
        case .loadFailed:
            return (isRetrying ? trying : tr("Read it again", "再读一次"), retry)
        case let .syncPaused(_, _, reason):
            switch reason?.remedy ?? .tryAgain {
            case .tryAgain:
                return (isRetrying ? trying : tr("Retry sync", "重试同步"), retry)
            case .openSettings:
                guard let openSettings else { return nil }
                return (tr("Open sync settings", "去同步设置"), openSettings)
            case .waitItOut:
                return nil
            }
        }
    }

    private var trying: String { tr("Trying", "正在重试") }

    private var reassurance: String {
        switch notice {
        case .syncPaused:
            total == 1
                ? tr(
                    "Your one snippet is on this device, and still works.",
                    "你那一枚片段就在这台设备上,照常用。"
                )
                : tr(
                    "All \(total) snippets are on this device, and still work.",
                    "\(total) 枚片段都在这台设备上,照常用。"
                )
        case .loadFailed:
            tr(
                "Nothing has been lost — your snippets are on this device.",
                "什么都没丢,片段都在这台设备上。"
            )
        }
    }

    private var problem: String {
        switch notice {
        case let .syncPaused(_, lastSyncAt, reason):
            let ago = lastSyncAt.map { RelativeStamp.text(for: $0, language: tr.language) }
                ?? tr("a while ago", "有一段时间了")
            // With a reason, the card can say why. Without one — a queue that
            // has simply not been sent yet — it states the queue and stops,
            // because the alternative is a cause it does not have.
            guard let reason else {
                return tr(
                    "Sync stopped \(ago). New changes queue up and go out when it comes back.",
                    "同步停在 \(ago)。新改动会排队,等连上了自己送出去。"
                )
            }
            return tr(
                "\(reason.cause(tr)) Sync stopped \(ago), and new changes queue up until it goes through.",
                "\(reason.cause(tr))同步停在 \(ago),新改动会一直排队,直到送出去为止。"
            )
        case .loadFailed:
            return tr(
                "The library could not be read just now. Nothing was written.",
                "刚才读不到本地库。没有写入任何东西。"
            )
        }
    }
}

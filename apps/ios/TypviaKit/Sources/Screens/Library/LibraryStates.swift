// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// The secret chapter, shut.
///
/// The material breaks: this page is an ink room in either theme. Nothing is
/// previewed, nothing is counted, and the rows are runs of dots whose lengths
/// are fixed — a dotted line that followed the real length would publish the
/// length of every secret in the vault.
struct SecretChapter: View {
    @Environment(\.tr) private var tr

    let chapter: Chapter
    let openVault: (() -> Void)?

    /// Fixed lengths, so the page looks like set type without describing any.
    private static let ruleWidths: [CGFloat] = [0.86, 0.52, 0.72, 0.36, 0.80]

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ChapterHead(chapter: chapter, onInkRoom: true)
                .padding(.bottom, LibraryMetrics.lockedTop)
            ForEach(Array(SecretChapter.ruleWidths.enumerated()), id: \.offset) { _, width in
                GeometryReader { proxy in
                    Text(BodyPreview.mask)
                        .typviaType(.mono)
                        .foregroundStyle(VaultRoom.ink3)
                        .lineLimit(1)
                        // Laid out whole and then cut, rather than truncated:
                        // a run of dots that ends in an ellipsis reads as text
                        // that was too long, which is a claim about the
                        // content this page is refusing to make.
                        .fixedSize(horizontal: true, vertical: false)
                        .frame(width: proxy.size.width * width, alignment: .leading)
                        .clipped()
                }
                .frame(height: LibraryMetrics.skeletonBodyHeight)
                .padding(.bottom, LibraryMetrics.lockedRowGap)
                .accessibilityHidden(true)
            }
            HStack(spacing: LibraryMetrics.lockedRowGap) {
                Caret(
                    height: LibraryMetrics.skeletonTitleHeight,
                    color: VaultRoom.accent
                )
                Text(tr("This chapter is shut. Nothing is set until it opens.", "这一章锁着。解锁后才排字。"))
                    .typviaType(.bodyS)
                    .foregroundStyle(VaultRoom.ink2)
            }
            .padding(.top, LibraryMetrics.lockedBodyGap)
            if let openVault {
                Button(action: openVault) {
                    Text(tr("Open the vault with Face ID", "用面容 ID 打开保险库"))
                        .typviaType(.bodyS)
                        .foregroundStyle(VaultRoom.accent)
                        .frame(minHeight: Tokens.Hit.minimum)
                }
                .buttonStyle(.plain)
                .padding(.top, LibraryMetrics.lockedActionGap)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, Tokens.Space.screenPadding)
        .padding(.top, LibraryMetrics.headerBottom)
        // The room carries its own ground rather than trusting whatever it was
        // placed on: an ink room drawn on a page is a page of invisible type.
        .background(VaultRoom.base)
        // The vault breathes at its own pace, and it is the room this page
        // belongs to whatever the app's theme is.
        .room(.vault)
    }
}

/// A chapter with nothing set in it yet.
struct EmptyChapter: View {
    @Environment(\.tr) private var tr

    let sort: TypeSort
    let compose: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(tr("This chapter is still empty.", "这一章还空着。"))
                .typviaType(.sectionTitle)
                .foregroundStyle(Paper.ink)
            Text(invitation)
                .typviaType(.body)
                .foregroundStyle(Paper.ink2)
                .padding(.top, LibraryMetrics.emptyBodyGap)
            if let compose {
                PressButton(title: newTitle, isPrimary: true, action: compose)
                    .padding(.top, LibraryMetrics.emptyActionsGap)
            }
        }
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// Each chapter invites in its own words. A shared "no items" sentence
    /// would be the one list template the design forbids, wearing a different
    /// heading.
    private var invitation: String {
        switch sort {
        case .text:
            tr(
                "Addresses, signatures, the reply you keep rewriting.",
                "地址、签名、那句你一再重写的回复。"
            )
        case .code:
            tr("Snippets you paste more than you type.", "那些你粘贴多过手打的代码。")
        case .command:
            tr("One-liners you look up every time.", "每次都要翻一遍的那几条命令。")
        case .prompt:
            tr("The instructions you give a model twice a day.", "你一天要给模型两遍的那些话。")
        case .template:
            tr("Text with holes in it, filled in as you insert.", "留了空位的文本,插入时再填。")
        case .secret:
            tr("Keys and passwords live in the vault.", "密钥与密码住在保险库里。")
        case .aiAction:
            tr(
                "An AI action is one instruction to a model — select text and it rewrites in place. Try saving one: \"make this shorter\".",
                "AI 动作是一句写给模型的指令,选中文字就能就地改写。先存一条试试:「把这段改写得更简短」。"
            )
        case .link:
            tr("Links you send often. Nothing is ever fetched.", "常发的链接。永远不联网抓取。")
        }
    }

    private var newTitle: String {
        switch sort {
        case .aiAction: tr("New AI action", "新建 AI 动作")
        default: tr("Save one", "存一条")
        }
    }
}

/// The chapter could not be read. One line, and the true half first.
struct UnavailableChapter: View {
    @Environment(\.tr) private var tr

    var body: some View {
        Text(
            tr(
                "Your snippets are on this device. This chapter could not be read just now.",
                "片段都在这台设备上。这一章刚才没读出来。"
            )
        )
        .typviaType(.bodyS)
        .foregroundStyle(Paper.ink2)
        .fixedSize(horizontal: false, vertical: true)
    }
}

/// The chapter's rows before they arrive: the compositor sets the measure
/// first, so nothing jumps when the words land.
struct ChapterSkeleton: View {
    @Environment(\.tr) private var tr

    var body: some View {
        VStack(alignment: .leading, spacing: LibraryMetrics.skeletonRowGap) {
            ForEach(0..<5, id: \.self) { index in
                VStack(alignment: .leading, spacing: LibraryMetrics.rowInnerGap) {
                    SkeletonLine(
                        width: LibraryMetrics.skeletonTitleWidth,
                        height: LibraryMetrics.skeletonTitleHeight,
                        delay: Double(index) * LibraryMetrics.skeletonStagger
                    )
                    SkeletonLine(
                        width: nil,
                        height: LibraryMetrics.skeletonBodyHeight,
                        delay: Double(index) * LibraryMetrics.skeletonStagger
                            + LibraryMetrics.skeletonStagger / 2
                    )
                }
            }
        }
        .accessibilityElement()
        .accessibilityLabel(tr("Setting this chapter", "正在排这一章"))
    }
}

/// One placeholder line. Shared with nothing: the home screen's skeleton is a
/// shelf of cards and this one is a column of rows, and pretending they are
/// the same component would be the shared list template the design forbids.
private struct SkeletonLine: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isDark = false

    let width: CGFloat?
    let height: CGFloat
    let delay: Double

    var body: some View {
        RoundedRectangle(cornerRadius: LibraryMetrics.skeletonRadius, style: .continuous)
            .fill(Paper.ink)
            .frame(maxWidth: width ?? .infinity)
            .frame(height: height)
            .opacity(isDark ? LibraryMetrics.skeletonPeak : LibraryMetrics.skeletonFloor)
            .onAppear {
                guard !reduceMotion else { return }
                withAnimation(
                    .easeInOut(duration: LibraryMetrics.skeletonBreath / 2)
                        .repeatForever(autoreverses: true)
                        .delay(delay)
                ) {
                    isDark = true
                }
            }
            .accessibilityHidden(true)
    }
}

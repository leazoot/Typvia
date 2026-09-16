// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// The search position at rest: the top of the screen given over to one line
/// that has not been written yet.
///
/// There is no field here, no border and no magnifying glass — a caret waiting
/// in front of a sentence, and a rule under where the words will go.
struct SearchPosition: View {
    @Environment(\.tr) private var tr

    let counts: HomeCounts
    let footnote: SearchFootnote

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Spacer(minLength: 0)
            Text(tr("Search", "搜索"))
                .typviaType(.caption)
                .foregroundStyle(Paper.ink3)
            HStack(spacing: HomeMetrics.searchCaretGap) {
                Caret(height: HomeMetrics.restingCaretHeight, capped: true)
                    .padding(.horizontal, HomeMetrics.capOverhangRoom)
                Text(tr("Search snippets", "搜索片段"))
                    .typviaType(.title1)
                    .foregroundStyle(Paper.ink3)
            }
            .padding(.top, HomeMetrics.searchLabelGap)
            SearchUnderline(isFocused: false)
                .padding(.top, HomeMetrics.searchRuleGap)
            footnoteRow
                .padding(.top, HomeMetrics.searchFootnoteGap)
        }
        .frame(height: HomeMetrics.searchBlockHeight, alignment: .bottom)
        .padding(.bottom, HomeMetrics.searchBlockBottom)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isSearchField)
    }

    @ViewBuilder
    private var footnoteRow: some View {
        switch footnote {
        case let .counts(lastSyncAt):
            HStack {
                Text(inventory)
                Spacer(minLength: HomeMetrics.searchCaretGap)
                Text(syncedWord(lastSyncAt))
            }
            .typviaType(.mono)
            .foregroundStyle(Paper.ink3)
        case .offline:
            HStack {
                Text(
                    tr.counted(counts.total, "snippet", "snippets", "\(counts.total) 枚")
                        + tr(" · all available", " · 全部可用")
                )
                Spacer(minLength: HomeMetrics.searchCaretGap)
                Text(tr("On this device", "本地"))
            }
            .typviaType(.mono)
            .foregroundStyle(Paper.ink3)
        case .loading:
            HStack(spacing: HomeMetrics.loadingLabelGap) {
                GrowingRule()
                Text(tr("Reading the local library", "正在读取本地库"))
                    .typviaType(.mono)
                    .foregroundStyle(Paper.ink3)
            }
        }
    }

    private var inventory: String {
        tr.counted(counts.total, "snippet", "snippets", "\(counts.total) 枚")
            + " · "
            + tr.counted(counts.sorts, "kind", "kinds", "\(counts.sorts) 类")
    }

    private func syncedWord(_ lastSyncAt: Int64?) -> String {
        guard let stamp = lastSyncAt else {
            return tr("Not synced yet", "尚未同步")
        }
        let ago = RelativeStamp.text(for: stamp, language: tr.language)
        return tr("Synced \(ago)", "同步于 \(ago)")
    }
}

/// What the line under the search rule says. All three are the same line in
/// the same place: the screen never grows or drops it, so nothing moves when
/// the state changes.
enum SearchFootnote: Equatable {
    case counts(lastSyncAt: Int64?)
    case offline
    case loading
}

/// The search position once the reader is in it: collapsed to a single mono
/// line with the caret in the text.
struct SearchLine: View {
    @Environment(\.tr) private var tr

    @Binding var query: String
    var isFocused: FocusState<Bool>.Binding
    let resultCount: Int
    let onCancel: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: HomeMetrics.searchLineGap) {
                field
                Spacer(minLength: HomeMetrics.searchLineGap)
                Button(action: onCancel) {
                    Text(tr("Cancel", "取消"))
                        .typviaType(.bodyS)
                        .foregroundStyle(Paper.ink2)
                        .frame(minHeight: Tokens.Hit.minimum)
                }
                .buttonStyle(.plain)
            }
            SearchUnderline(isFocused: true)
                .padding(.top, HomeMetrics.searchLineRuleGap)
            Text(matchLine)
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
                .padding(.top, HomeMetrics.searchFootnoteGap)
        }
    }

    private var field: some View {
        TextField("", text: $query)
            .focused(isFocused)
            // The field takes the focus when it arrives rather than being
            // handed it before it exists. Whoever asked for the search line —
            // a tap on the position above, or the pull — set a flag; the flag
            // is what put this view on screen, and a `@FocusState` written
            // while the field was still absent is one SwiftUI drops on the
            // floor. That is why both ways in did nothing at all.
            .onAppear { isFocused.wrappedValue = true }
            .typviaType(.searchInput)
            .foregroundStyle(Paper.ink)
            // The system caret is the caret: it blinks the way the frames
            // draw it and it is the only one that knows where composed text
            // is going. Tinting it is what makes it ours.
            .tint(Room.home.accent)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .submitLabel(.search)
            .accessibilityLabel(tr("Search snippets", "搜索片段"))
    }

    private var matchLine: String {
        guard !query.trimmingCharacters(in: .whitespaces).isEmpty else {
            return tr("Type to search", "开始输入")
        }
        return tr.counted(resultCount, "match", "matches", "\(resultCount) 枚匹配")
            + tr(" · closest first", " · 最贴合的排前")
    }
}

/// One result: mark, title, a glimpse of the body, and the trigger that
/// matched — or, for a secret, the fact that it will not be shown.
struct ResultRow: View {
    @Environment(\.tr) private var tr

    let result: SearchResult

    var body: some View {
        HStack(alignment: .top, spacing: HomeMetrics.rowGap) {
            if let sort = result.sort {
                TypeSortMark(sort, accessibilityLabel: sortName(sort))
                    .padding(.top, HomeMetrics.markLift)
            }
            VStack(alignment: .leading, spacing: HomeMetrics.rowTitleGap) {
                Text(result.title)
                    .typviaType(.sectionTitle)
                    .foregroundStyle(Paper.ink)
                preview
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            tail
        }
        .padding(.vertical, HomeMetrics.rowPadding)
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private var preview: some View {
        switch result.preview {
        case let .text(body):
            Text(body)
                .typviaType(.mono)
                .foregroundStyle(Paper.ink2)
                .lineLimit(1)
                .truncationMode(.tail)
                // The body is a glimpse for the eye, not something to read
                // aloud: the title and the trigger are the row.
                .accessibilityHidden(true)
        case .withheld:
            Text(BodyPreview.mask)
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
                .lineLimit(1)
                .accessibilityLabel(tr("Content is not shown", "内容不展示"))
        }
    }

    @ViewBuilder
    private var tail: some View {
        switch result.tail {
        case let .trigger(match):
            (Text(match.matched).underline(true, color: Room.home.accent) + Text(match.rest))
                .typviaType(.mono)
                .foregroundStyle(Room.home.accent)
                .padding(.top, HomeMetrics.tailLift)
        case .needsVerification:
            Text(tr("Verify first", "需验证"))
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
                .padding(.top, HomeMetrics.tailLift)
        case .none:
            EmptyView()
        }
    }

    private func sortName(_ sort: TypeSort) -> String {
        switch sort {
        case .text: tr("Text", "文本")
        case .code: tr("Code", "代码")
        case .command: tr("Command", "命令")
        case .prompt: tr("Prompt", "提示词")
        case .template: tr("Template", "模板")
        case .secret: tr("Secret", "密钥")
        case .aiAction: tr("AI action", "AI 动作")
        case .link: tr("Link", "链接")
        }
    }
}

/// The reading indicator: an accent rule that grows and retreats. It is the
/// product's spinner, and it is not a spinner.
struct GrowingRule: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isGrown = false

    var body: some View {
        Rectangle()
            .fill(Room.library.accent)
            .frame(width: HomeMetrics.growingRuleWidth, height: HomeMetrics.growingRuleHeight)
            .scaleEffect(x: isGrown ? 1 : 0, anchor: .leading)
            .accessibilityHidden(true)
            .onAppear {
                guard !reduceMotion else {
                    isGrown = true
                    return
                }
                withAnimation(
                    Curve.enter(Tokens.Motion.sweepDuration).repeatForever(autoreverses: true)
                ) {
                    isGrown = true
                }
            }
    }
}

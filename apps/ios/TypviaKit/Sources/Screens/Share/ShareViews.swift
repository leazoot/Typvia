// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// The share sheet: everything already set, two steps at most.
public struct ShareSheet: View {
    @Environment(\.tr) private var tr

    @Binding private var draft: SharedDraft
    private let step: ShareStep
    private let save: (() -> Void)?
    private let cancel: (() -> Void)?
    private let edit: (() -> Void)?
    private let back: (() -> Void)?

    public init(
        draft: Binding<SharedDraft>,
        step: ShareStep,
        save: (() -> Void)? = nil,
        cancel: (() -> Void)? = nil,
        edit: (() -> Void)? = nil,
        back: (() -> Void)? = nil
    ) {
        _draft = draft
        self.step = step
        self.save = save
        self.cancel = cancel
        self.edit = edit
        self.back = back
    }

    public var body: some View {
        Group {
            if case let .saved(trigger, position) = step {
                SavedNote(trigger: trigger, position: position, edit: edit, back: back)
            } else {
                composer
            }
        }
        .padding(Tokens.Space.screenPadding)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Paper.base)
        .room(.home)
    }

    private var composer: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(draft.body)
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink)
                .lineLimit(ShareMetrics.bodyLines)
                .fixedSize(horizontal: false, vertical: true)
            field(tr("Title", "标题")) {
                Text(draft.title)
                    .typviaType(.sectionTitle)
                    .foregroundStyle(Paper.ink)
            }
            field(tr("Trigger", "触发词")) {
                Text(draft.trigger)
                    .typviaType(.mono)
                    .foregroundStyle(Room.home.accent)
            }
            sorts
                .padding(.top, ShareMetrics.fieldGap)
            Text(tr("Guessed from the text as \(draft.sort.name(tr))", "已按内容猜为\(draft.sort.name(tr))"))
                .typviaType(.caption)
                .foregroundStyle(Paper.ink3)
                .padding(.top, ShareMetrics.hintGap)
            HStack(spacing: ShareMetrics.actionGap) {
                if let save {
                    ShareVerb(title: tr("Save", "存下"), isPrimary: true, action: save)
                }
                if let cancel {
                    ShareVerb(title: tr("Cancel", "取消"), action: cancel)
                }
            }
            .padding(.top, ShareMetrics.actionGap)
        }
    }

    /// Four kinds, the rest a swipe away. More than four turns the strip into
    /// a second keyboard on a sheet that is supposed to be two steps.
    private var sorts: some View {
        HStack(spacing: ShareMetrics.sortGap) {
            ForEach(SharedDraft.offered, id: \.self) { sort in
                Button { draft.sort = sort } label: {
                    TypeSortMark(
                        sort,
                        size: .compact,
                        inverted: draft.sort == sort,
                        accessibilityLabel: sort.name(tr)
                    )
                }
                .buttonStyle(.plain)
            }
        }
    }

    private func field(_ label: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: ShareMetrics.labelGap) {
            Text(label)
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.ink3)
            content()
        }
        .padding(.top, ShareMetrics.fieldGap)
    }
}

/// Saved. No celebration — one sentence teaching what the trigger now does,
/// which for most readers is the first time they learn what this product is
/// for. The default way out goes back where they came from.
struct SavedNote: View {
    @Environment(\.tr) private var tr

    let trigger: String
    let position: UInt32
    let edit: (() -> Void)?
    let back: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Caret(height: ShareMetrics.caretHeight)
            Text(tr("Saved · number \(position)", "已存 · 第 \(position) 枚"))
                .typviaType(.title2)
                .foregroundStyle(Paper.ink)
                .padding(.top, ShareMetrics.fieldGap)
            Text(trigger)
                .typviaType(.mono)
                .foregroundStyle(Room.home.accent)
                .padding(.top, ShareMetrics.labelGap)
            Text(
                tr(
                    "Type \(trigger) in any text field and it comes back on its own.",
                    "下次在任何输入框里打 \(trigger),它就会自己出来。"
                )
            )
            .typviaType(.bodyS)
            .foregroundStyle(Paper.ink2)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, ShareMetrics.fieldGap)
            HStack(spacing: ShareMetrics.actionGap) {
                if let back {
                    ShareVerb(title: tr("Go back", "回到刚才"), isPrimary: true, action: back)
                }
                if let edit {
                    ShareVerb(title: tr("Change it", "改一下"), action: edit)
                }
            }
            .padding(.top, ShareMetrics.actionGap)
        }
    }
}

/// A word on the sheet. The share extension keeps its own so that it links no
/// screen from the app.
struct ShareVerb: View {
    let title: String
    var isPrimary = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .typviaType(isPrimary ? .sectionTitle : .bodyS)
                .foregroundStyle(isPrimary ? Paper.ink : Paper.ink2)
                .frame(minHeight: Tokens.Hit.minimum)
        }
        .buttonStyle(.plain)
    }
}

/// A widget's contents: titles and marks, never a body.
public struct WidgetFace: View {
    @Environment(\.tr) private var tr

    private let size: WidgetSize
    private let rows: [WidgetRow]
    private let total: UInt32

    public init(size: WidgetSize, rows: [WidgetRow], total: UInt32) {
        self.size = size
        self.rows = rows
        self.total = total
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: ShareMetrics.widgetRowGap) {
            header
            if size.showsRows {
                ForEach(rows.prefix(size.rowLimit)) { row in
                    widgetRow(row)
                }
            }
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Paper.base)
        .room(.home)
    }

    private var header: some View {
        HStack(spacing: ShareMetrics.labelGap) {
            // Static, not breathing: widgets get no animation, so the caret is
            // simply present.
            Caret(height: ShareMetrics.widgetCaretHeight, behaviour: .still)
            Text(tr("Search snippets", "搜索片段"))
                .typviaType(.mono)
                .foregroundStyle(Paper.ink2)
            Spacer(minLength: 0)
            Text(tr("\(total)", "\(total) 枚"))
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
        }
    }

    private func widgetRow(_ row: WidgetRow) -> some View {
        HStack(spacing: ShareMetrics.labelGap) {
            if let sort = row.sort {
                TypeSortMark(sort, size: .compact, accessibilityLabel: sort.name(tr))
            }
            Text(row.title)
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink)
                .lineLimit(1)
            Spacer(minLength: 0)
            Text(row.isSecret ? tr("verify", "需验证") : (row.trigger ?? ""))
                .typviaType(.mono)
                .foregroundStyle(row.isSecret ? Paper.ink3 : Room.home.accent)
        }
        .accessibilityElement(children: .combine)
    }
}

enum ShareMetrics {
    static let bodyLines = 3
    static let fieldGap: CGFloat = 18
    static let labelGap: CGFloat = 6
    static let sortGap: CGFloat = 8
    static let hintGap: CGFloat = 10
    static let actionGap: CGFloat = 22
    static let caretHeight: CGFloat = 34
    static let widgetRowGap: CGFloat = 12
    static let widgetCaretHeight: CGFloat = 14
}

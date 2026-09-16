// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// A chapter's head: the oversized sort that is the page, its name, and how
/// many pieces of type are in the case.
struct ChapterHead: View {
    @Environment(\.tr) private var tr

    let chapter: Chapter
    /// The shut chapter is an ink room, and everything on it takes the room's
    /// colours rather than the page's.
    var onInkRoom = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            TypeSortMark(
                chapter.sort,
                size: .chapter,
                inverted: chapter.sort == .secret,
                on: onInkRoom ? .inkRoom : .page,
                accessibilityLabel: chapter.sort.name(tr)
            )
            // An empty chapter's mark fades rather than disappearing: the
            // chapter exists, it just has no type set in it yet.
            .opacity(markOpacity)
            Text(chapter.sort.name(tr))
                .typviaType(.heading)
                .foregroundStyle(onInkRoom ? VaultRoom.ink : Paper.ink)
                .padding(.top, LibraryMetrics.headNameGap)
            countLine
                .padding(.top, LibraryMetrics.headCountGap)
        }
    }

    @ViewBuilder
    private var countLine: some View {
        if chapter.showsCount {
            Text(tr.counted(chapter.count, "piece", "pieces", "\(chapter.count) 枚"))
                .typviaType(.mono)
                .foregroundStyle(onInkRoom ? VaultRoom.ink3 : Paper.ink3)
        } else {
            Text(BodyPreview.shortMask)
                .typviaType(.mono)
                .foregroundStyle(VaultRoom.ink3)
                .accessibilityLabel(tr("Not counted", "不计数"))
        }
    }

    private var markOpacity: Double {
        switch chapter.phase {
        case .empty: LibraryMetrics.emptyMarkOpacity
        case .loading: LibraryMetrics.loadingMarkOpacity
        case .listed, .locked, .unavailable: 1
        }
    }
}

/// One row: title and trigger on a line, a glimpse of the body under it, and
/// no rule anywhere. Grouping is done with space.
struct ChapterRow: View {
    @Environment(\.tr) private var tr

    let row: LibraryRow
    let style: ChapterStyle
    let open: ((String) -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: LibraryMetrics.rowInnerGap) {
            HStack(alignment: .firstTextBaseline, spacing: LibraryMetrics.rowTitleGap) {
                Text(row.title)
                    .typviaType(.sectionTitle)
                    .foregroundStyle(Paper.ink)
                if let trigger = row.trigger {
                    Text(trigger)
                        .typviaType(.mono)
                        .foregroundStyle(style.accent.accent)
                }
                Spacer(minLength: 0)
                if let slots = row.slots {
                    Text(tr.counted(slots, "blank", "blanks", "\(slots) 个空位"))
                        .typviaType(.mono)
                        .foregroundStyle(style.accent.accent)
                }
            }
            preview
        }
        .padding(.vertical, LibraryMetrics.rowPadding)
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
        .onTapGesture { open?(row.id) }
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private var preview: some View {
        switch row.preview {
        case let .text(body):
            Text(body)
                .typviaType(.mono)
                .foregroundStyle(Paper.ink2)
                .lineLimit(style.truncates ? style.previewLines : nil)
                .truncationMode(.tail)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(gridPaper)
                .accessibilityHidden(true)
        case .withheld:
            Text(BodyPreview.mask)
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
                .lineLimit(1)
                .accessibilityLabel(tr("Content is not shown", "内容不展示"))
        }
    }

    /// The code chapter sets its bodies over a grid so faint it reads as
    /// texture rather than as a table. No syntax colouring anywhere: this is a
    /// specimen book, not an editor.
    @ViewBuilder
    private var gridPaper: some View {
        if style.material == .grid {
            GridPaper()
        }
    }
}

/// A folder's name over the rows filed under it. It is a label, not a row:
/// mono, small, spaced out, and carrying nothing tappable.
struct GroupHeading: View {
    let title: String

    var body: some View {
        Text(title)
            .typviaType(.monoLabel)
            .foregroundStyle(Paper.ink3)
            .textCase(.uppercase)
            .padding(.top, LibraryMetrics.groupHeadingTop)
            .padding(.bottom, LibraryMetrics.groupHeadingBottom)
            .accessibilityAddTraits(.isHeader)
    }
}

/// A ruled ground for mono type: two sets of hairlines at the same weight the
/// rest of the product uses for a rule, thinned again so they never read as
/// lines of their own.
struct GridPaper: View {
    var body: some View {
        GeometryReader { proxy in
            Path { path in
                var x = LibraryMetrics.gridStep
                while x < proxy.size.width {
                    path.move(to: CGPoint(x: x, y: 0))
                    path.addLine(to: CGPoint(x: x, y: proxy.size.height))
                    x += LibraryMetrics.gridStep
                }
                var y = LibraryMetrics.gridStep
                while y < proxy.size.height {
                    path.move(to: CGPoint(x: 0, y: y))
                    path.addLine(to: CGPoint(x: proxy.size.width, y: y))
                    y += LibraryMetrics.gridStep
                }
            }
            .stroke(Paper.rule, lineWidth: Tokens.Line.hairlineWidth)
            .opacity(LibraryMetrics.gridOpacity)
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// The sort bar: eight pieces of type along the bottom, which are at once the
/// table of contents and the progress indicator. It never goes into a
/// skeleton — wherever the reader is, they can always leave.
struct SortBar: View {
    @Environment(\.tr) private var tr

    let current: TypeSort
    /// Kept on the shut chapter too, in the room's colours. The frame drops it
    /// there; leaving a chapter must not depend on discovering a swipe.
    var onInkRoom = false
    let select: (TypeSort) -> Void

    var body: some View {
        HStack(spacing: LibraryMetrics.sortBarGap) {
            ForEach(Chapter.order, id: \.self) { sort in
                Button {
                    select(sort)
                } label: {
                    Text(sort.code)
                        .typviaType(.monoLabel)
                        .foregroundStyle(letterInk(for: sort))
                        .frame(minWidth: LibraryMetrics.sortBarItem, minHeight: Tokens.Hit.minimum)
                        .overlay(alignment: .bottom) { marker(for: sort) }
                }
                .buttonStyle(.plain)
                .accessibilityLabel(sort.name(tr))
                .accessibilityAddTraits(sort == current ? [.isSelected] : [])
            }
        }
    }

    @ViewBuilder
    private func marker(for sort: TypeSort) -> some View {
        if sort == current {
            Rectangle()
                .fill(onInkRoom ? VaultRoom.accent : Room.library.accent)
                .frame(height: LibraryMetrics.sortBarMarker)
        }
    }

    private func letterInk(for sort: TypeSort) -> Color {
        if sort == current {
            return onInkRoom ? VaultRoom.ink : Paper.ink
        }
        return onInkRoom ? VaultRoom.ink3 : Paper.ink3
    }
}

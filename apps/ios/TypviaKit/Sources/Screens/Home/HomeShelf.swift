// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation
import SwiftUI

/// What sits under the search position when nobody is searching: the sheets
/// that were used last, and the trigger words that get typed most.
struct ShelfView: View {
    @Environment(\.tr) private var tr

    let tiles: [RecallTile]
    let triggers: [TriggerLine]
    let total: UInt32
    let open: ((String) -> Void)?
    /// The way to write another one. It used to live only in the empty state,
    /// which meant the product could be used to save exactly one snippet and
    /// then never again: the invitation disappeared with the blank sheet it
    /// was printed on.
    var compose: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            cards
            if let compose, total > 0 {
                Button(action: compose) {
                    Text(tr("Write one", "写一枚"))
                        .typviaType(.sectionTitle)
                        .foregroundStyle(Paper.ink)
                }
                .buttonStyle(.plain)
                .padding(.top, Tokens.Space.group)
            }
            if !triggers.isEmpty {
                Text(tr("Trigger words", "常用触发词"))
                    .typviaType(.sectionTitle)
                    .foregroundStyle(Paper.ink)
                    .padding(.top, Tokens.Space.group)
                    .padding(.trailing, Tokens.Space.screenPadding)
                triggerList
            }
        }
    }

    private var header: some View {
        HStack(alignment: .firstTextBaseline) {
            // The heading belongs to the cards under it. A library whose
            // snippets have never been used has no recall order yet, and
            // "Used recently" over an empty strip is a heading for nothing —
            // the count on the right still says what is there.
            if !tiles.isEmpty {
                Text(tr("Used recently", "最近用过"))
                    .typviaType(.sectionTitle)
                    .foregroundStyle(Paper.ink)
            }
            Spacer(minLength: 0)
            Text(tr("All \(total)", "全部 \(total)"))
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
        }
        .padding(.trailing, Tokens.Space.screenPadding)
        .padding(.bottom, HomeMetrics.shelfHeaderGap)
    }

    /// A run of sheets that carries on past the right edge — the frames stop
    /// the third card halfway, which is the whole hint that there are more.
    private var cards: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: HomeMetrics.cardGap) {
                ForEach(tiles) { tile in
                    RecallCard(tile: tile, open: open)
                }
            }
            .padding(.trailing, Tokens.Space.screenPadding)
            // Room for the resting shadow, which a clipping scroll view would
            // otherwise cut off along the top and bottom edges.
            .padding(.vertical, HomeMetrics.shadowRoom)
        }
    }

    private var triggerList: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(triggers) { line in
                TriggerRow(line: line, open: open)
            }
        }
        .padding(.trailing, Tokens.Space.screenPadding)
    }
}

/// One recall card: a sheet with a sort pressed into its corner.
struct RecallCard: View {
    @Environment(\.tr) private var tr
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isPickedUp = false

    let tile: RecallTile
    let open: ((String) -> Void)?

    var body: some View {
        PaperCard(edge: isPickedUp ? .lifted : .resting) {
            VStack(alignment: .leading, spacing: 0) {
                if let sort = tile.sort {
                    TypeSortMark(sort)
                }
                Text(tile.title)
                    .typviaType(.sectionTitle)
                    .foregroundStyle(Paper.ink)
                    .lineLimit(2)
                    .padding(.top, HomeMetrics.cardTitleGap)
                if let trigger = tile.trigger {
                    Text(trigger)
                        .typviaType(.mono)
                        .foregroundStyle(Room.home.accent)
                        .lineLimit(1)
                        .padding(.top, HomeMetrics.cardTriggerGap)
                }
                Spacer(minLength: HomeMetrics.cardTriggerGap)
                Text(
                    tile.usageCount == 1
                        ? tr("Used once", "用过 1 次")
                        : tr("Used \(tile.usageCount) times", "用过 \(tile.usageCount) 次")
                )
                    .typviaType(.mono)
                    .foregroundStyle(Paper.ink3)
            }
            .padding(HomeMetrics.cardPadding)
            .frame(width: HomeMetrics.cardWidth, height: HomeMetrics.cardHeight, alignment: .topLeading)
        }
        .pickedUp(isPickedUp)
        .onLongPressGesture(minimumDuration: HomeMetrics.pickUpDelay) {
            // Picking a sheet up is the gesture that will hand it to another
            // screen. Until there is somewhere to hand it to, it does what the
            // frames show it doing and no more: it lifts.
            isPickedUp = true
        } onPressingChanged: { isPressing in
            if !isPressing { isPickedUp = false }
        }
        .onTapGesture { open?(tile.id) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(open == nil ? [] : .isButton)
    }
}

/// One trigger line: the word on the left, what it types on the right.
struct TriggerRow: View {
    @Environment(\.tr) private var tr

    let line: TriggerLine
    let open: ((String) -> Void)?

    var body: some View {
        HStack(spacing: HomeMetrics.triggerGap) {
            triggerColumn
            Text(title)
                .typviaType(.bodyS)
                .foregroundStyle(isLockedVault ? Paper.ink2 : Paper.ink)
                .frame(maxWidth: .infinity, alignment: .leading)
            sortColumn
        }
        .padding(.vertical, HomeMetrics.triggerRowPadding)
        .frame(minHeight: Tokens.Hit.minimum)
        .contentShape(Rectangle())
        .onTapGesture {
            guard case .snippet = line.kind else { return }
            open?(line.id)
        }
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private var triggerColumn: some View {
        switch line.kind {
        case let .snippet(trigger):
            Text(trigger)
                .typviaType(.mono)
                .foregroundStyle(Room.home.accent)
                .lineLimit(1)
                .frame(width: HomeMetrics.triggerColumnWidth, alignment: .leading)
        case .lockedVault:
            Text(BodyPreview.shortMask)
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
                .frame(width: HomeMetrics.triggerColumnWidth, alignment: .leading)
                .accessibilityHidden(true)
        }
    }

    @ViewBuilder
    private var sortColumn: some View {
        switch line.kind {
        case .snippet:
            // A letter pair, not a mark: three marks in a row would turn a
            // quiet list into a column of stamps.
            if let sort = line.sort {
                Text(sort.code)
                    .typviaType(.monoLabel)
                    .foregroundStyle(Paper.ink3)
                    .accessibilityHidden(true)
            }
        case .lockedVault:
            TypeSortMark(.secret, size: .compact, accessibilityLabel: tr("Secret", "密钥"))
        }
    }

    private var isLockedVault: Bool {
        if case .lockedVault = line.kind { return true }
        return false
    }

    private var title: String {
        isLockedVault ? tr("Vault is locked", "保险库已锁定") : line.title
    }
}

/// How long ago something happened, in words.
///
/// The frames write it as "2 分钟前"; the system formatter already knows how
/// to say that in both languages, and knows the plural rules this product
/// would otherwise have to invent.
enum RelativeStamp {
    static func text(for millis: Int64, language: UiLanguage, now: Date = Date()) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.locale = Locale(identifier: language == .zh ? "zh-Hans" : "en")
        formatter.unitsStyle = .full
        return formatter.localizedString(
            for: Date(timeIntervalSince1970: Double(millis) / 1000), relativeTo: now
        )
    }
}

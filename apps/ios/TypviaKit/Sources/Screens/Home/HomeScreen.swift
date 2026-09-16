// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// What the home screen can hand off to the rest of the app.
///
/// Every one of these is optional, and an absent handler means the affordance
/// is not drawn at all. The rooms this screen leads to arrive one stage at a
/// time, and a button that goes nowhere is worse than a button that is not
/// there yet.
public struct HomeActions {
    public var open: ((String) -> Void)?
    public var compose: (() -> Void)?
    public var importClipboard: (() -> Void)?
    public var selectRoom: ((Room) -> Void)?
    /// The rooms that have screens. The bar prints all of them and opens only
    /// these.
    public var availableRooms: Set<Room>

    public init(
        open: ((String) -> Void)? = nil,
        compose: (() -> Void)? = nil,
        importClipboard: (() -> Void)? = nil,
        selectRoom: ((Room) -> Void)? = nil,
        availableRooms: Set<Room> = []
    ) {
        self.open = open
        self.compose = compose
        self.importClipboard = importClipboard
        self.selectRoom = selectRoom
        self.availableRooms = availableRooms
    }
}

/// Home is search.
///
/// The top of the screen is a sheet of paper waiting to be written on: one
/// line, a rule, and two numbers. What is underneath — the sheets used last,
/// the trigger words, a notice, a skeleton, or nothing at all — depends on
/// which of the six states the screen is in, but the search position is in the
/// same place in every one of them, so it never moves while the reader is
/// looking at it.
public struct HomeScreen: View {
    @StateObject private var model: HomeModel
    @FocusState private var isSearchFocused: Bool
    /// The reader has asked for the search line. Separate from the focus,
    /// because the focus cannot be set until the line it belongs to exists.
    @State private var isAsking = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private let actions: HomeActions

    public init(store: TypviaStore, actions: HomeActions = HomeActions()) {
        _model = StateObject(wrappedValue: HomeModel(store: store))
        self.actions = actions
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            wordmark
            // The empty sheet is the one state with no search position: there
            // is nothing to search, and offering a line for it would be the
            // screen asking the reader for something it knows they do not
            // have yet.
            if model.phase != .empty {
                searchPosition
            }
            content
            Spacer(minLength: 0)
            RoomBar(current: .home, available: actions.availableRooms, select: actions.selectRoom)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Paper.base)
        .room(.home)
        .task { await model.load() }
    }

    private var wordmark: some View {
        Text("TYPVIA")
            .typviaType(.monoLabel)
            .foregroundStyle(Paper.ink3)
            .padding(.horizontal, Tokens.Space.screenPadding)
            .padding(.top, HomeMetrics.wordmarkTop)
    }

    /// The search position, in whichever of its two shapes applies. The
    /// collapse from one to the other is a transition rather than a swap, so
    /// the rule underneath stays put while the line above it shrinks.
    @ViewBuilder
    private var searchPosition: some View {
        Group {
            if isTyping {
                SearchLine(
                    query: $model.query,
                    isFocused: $isSearchFocused,
                    resultCount: model.results.count,
                    onCancel: cancelSearch
                )
                .padding(.top, HomeMetrics.searchLineTop)
            } else {
                SearchPosition(counts: model.counts, footnote: footnote)
                    .contentShape(Rectangle())
                    .onTapGesture { isAsking = true }
            }
        }
        .padding(.horizontal, Tokens.Space.screenPadding)
        .animation(Beat.transition.enter(reduceMotion: reduceMotion), value: isTyping)
    }

    @ViewBuilder
    private var content: some View {
        switch model.phase {
        case .searching:
            results
        case .loading:
            SkeletonShelf()
                .padding(.leading, Tokens.Space.screenPadding)
                .padding(.top, Tokens.Space.group)
        case .empty:
            VStack(spacing: 0) {
                Spacer(minLength: 0)
                EmptySheet(actions: actions)
                Spacer(minLength: 0)
            }
            // Sits above the middle, in the half of the screen a thumb
            // reaches, as the frame draws it.
            .padding(.bottom, HomeMetrics.emptyBottomBias)
            .frame(maxHeight: .infinity)
        case .resting:
            restingContent
        }
    }

    /// The shelf scrolls and the search position does not. The frames fit a
    /// full shelf on a 390×844 screen, but a longer library, a taller reader's
    /// type step or a shorter phone must push the shelf rather than the search
    /// line — and must never push it up under the clock.
    private var restingContent: some View {
        PullAwareScroll {
            shelf
        }
    }

    @ViewBuilder
    private var shelf: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let notice = model.notice {
                NoticeCard(
                    notice: notice,
                    total: model.counts.total,
                    isRetrying: model.isRetrying,
                    retry: { Task { await model.retry() } },
                    openSettings: settingsRoute
                )
                .padding(.horizontal, Tokens.Space.screenPadding)
                .padding(.bottom, Tokens.Space.group)
            }
            ShelfView(
                tiles: model.tiles,
                triggers: model.triggers,
                total: model.counts.total,
                open: actions.open,
                compose: actions.compose
            )
            .padding(.leading, Tokens.Space.screenPadding)
        }
        .padding(.top, HomeMetrics.shelfTop)
    }

    private var results: some View {
        ScrollView(showsIndicators: false) {
            VStack(alignment: .leading, spacing: 0) {
                ForEach(model.results) { result in
                    ResultRow(result: result)
                        .contentShape(Rectangle())
                        .onTapGesture { actions.open?(result.id) }
                }
            }
            .padding(.horizontal, Tokens.Space.screenPadding)
        }
        .padding(.top, HomeMetrics.resultsTop)
    }

    /// One line, three things it can say. Offline is not a separate layout:
    /// the counts line reports that everything is available locally, which is
    /// the reassurance the state is about.
    private var footnote: SearchFootnote {
        if model.phase == .loading { return .loading }
        if case .syncPaused = model.notice { return .offline }
        return .counts(lastSyncAt: model.lastSyncAt)
    }

    private var isTyping: Bool {
        isAsking || isSearchFocused || !model.query.isEmpty
    }

    /// The way to the settings room, when this host has one. Same rule as the
    /// room bar: a verb is offered only where it has somewhere to go.
    private var settingsRoute: (() -> Void)? {
        guard let selectRoom = actions.selectRoom,
              actions.availableRooms.contains(.settings)
        else { return nil }
        return { selectRoom(.settings) }
    }

    private func cancelSearch() {
        model.query = ""
        isAsking = false
        isSearchFocused = false
    }
}

/// What the pulled-back sheet used to reveal: a caret and one line, telling
/// the reader what letting go would do.
///
/// Kept, unused, for the moment a screen has a search position that can scroll
/// away — this one does not. On home the position is the top third of every
/// state, so pulling revealed a line already in front of the reader; and both
/// it and the tap set a focus before the field existed, so neither did
/// anything at all. One way in, and it works.
private struct PullHint: View {
    @Environment(\.tr) private var tr

    let isPastThreshold: Bool

    var body: some View {
        VStack(spacing: HomeMetrics.pullHintGap) {
            Caret(height: HomeMetrics.pullHintCaretHeight, behaviour: .blinking)
            Text(tr("Let go to pull out the search line", "松手,把搜索位抽出来"))
                .typviaType(.mono)
                .foregroundStyle(Paper.ink2)
        }
        .opacity(isPastThreshold ? 1 : HomeMetrics.pullHintDim)
        .accessibilityHidden(true)
    }
}

/// Measurements this screen takes from its own frames.
///
/// Anything shared with other screens — colour, type, radius, elevation,
/// motion, spacing rungs — comes from `Tokens`. What is here is the home
/// screen's own composition: the height of the block given to search, the
/// width of a recall card, the column a trigger word sits in.
enum HomeMetrics {
    static let wordmarkTop: CGFloat = 22

    /// The whole upper third, given to one unwritten line.
    static let searchBlockHeight: CGFloat = 244
    static let searchBlockBottom: CGFloat = 18
    static let searchLabelGap: CGFloat = 16
    static let searchRuleGap: CGFloat = 20
    static let searchFootnoteGap: CGFloat = 11
    static let searchCaretGap: CGFloat = 11
    static let restingCaretHeight: CGFloat = 38
    /// The resting caret wears crossbars, which stick out past its own width.
    static let capOverhangRoom: CGFloat = 6
    static let searchLineTop: CGFloat = 14
    static let searchLineGap: CGFloat = 3
    static let searchLineRuleGap: CGFloat = 12
    static let loadingLabelGap: CGFloat = 10
    static let growingRuleWidth: CGFloat = 64
    static let growingRuleHeight: CGFloat = 2

    static let resultsTop: CGFloat = 0
    static let rowGap: CGFloat = 13
    static let rowTitleGap: CGFloat = 5
    static let rowPadding: CGFloat = 18
    static let markLift: CGFloat = 2
    static let tailLift: CGFloat = 3

    static let shelfTop: CGFloat = 26
    static let shelfHeaderGap: CGFloat = 14
    static let cardWidth: CGFloat = 156
    static let cardHeight: CGFloat = 150
    static let cardGap: CGFloat = 12
    static let cardPadding: CGFloat = 14
    static let cardTitleGap: CGFloat = 12
    static let cardTriggerGap: CGFloat = 6
    static let shadowRoom: CGFloat = 12
    static let pickUpDelay: Double = 0.4

    static let triggerGap: CGFloat = 14
    static let triggerColumnWidth: CGFloat = 82
    static let triggerRowPadding: CGFloat = 15

    static let emptyInset: CGFloat = 40
    static let emptyBottomBias: CGFloat = 90
    static let emptyCaretHeight: CGFloat = 52
    static let emptyCaretOverhangRoom: CGFloat = 8
    static let emptyBodyGap: CGFloat = 14
    static let emptyButtonsGap: CGFloat = 32
    static let emptyAsideGap: CGFloat = 26
    static let buttonGap: CGFloat = 12
    static let buttonPaddingX: CGFloat = 22
    static let buttonPaddingY: CGFloat = 15
    static let buttonPressScale: CGFloat = 0.98

    /// The two weights the skeleton breathes between, and how long one full
    /// breath takes.
    static let skeletonFloor = 0.12
    static let skeletonPeak = 0.26
    static let skeletonBreath = 1.400
    static let skeletonStagger = 0.120
    static let skeletonRadius: CGFloat = 2
    static let skeletonHeadWidth: CGFloat = 88
    static let skeletonHeadHeight: CGFloat = 15
    static let skeletonGap: CGFloat = 18
    static let skeletonRowGap: CGFloat = 22
    static let skeletonRowHeight: CGFloat = 13
    static let skeletonTriggerWidth: CGFloat = 62

    static let noticePadding: CGFloat = 20
    static let noticeLabelGap: CGFloat = 9
    static let noticeLabelBottomGap: CGFloat = 13
    static let noticeTickWidth: CGFloat = 2
    static let noticeTickHeight: CGFloat = 12
    static let noticeBodyGap: CGFloat = 8
    static let noticeActionsGap: CGFloat = 18
    static let noticeActionGap: CGFloat = 20
    static let noticeUnderline: CGFloat = 1.5
    static let noticeUnderlineDrop: CGFloat = 2

    static let pullHintGap: CGFloat = 8
    static let pullHintCaretHeight: CGFloat = 20
    static let pullHintDim = 0.55
}

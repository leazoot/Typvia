// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// What the library can hand off. As on the home screen, an absent handler
/// means the affordance is not drawn.
public struct LibraryActions {
    public var open: ((String) -> Void)?
    public var openVault: (() -> Void)?
    public var compose: (() -> Void)?
    public var selectRoom: ((Room) -> Void)?
    /// The rooms that have screens. The bar prints all of them and opens only
    /// these.
    public var availableRooms: Set<Room>

    public init(
        open: ((String) -> Void)? = nil,
        openVault: (() -> Void)? = nil,
        compose: (() -> Void)? = nil,
        selectRoom: ((Room) -> Void)? = nil,
        availableRooms: Set<Room> = []
    ) {
        self.open = open
        self.openVault = openVault
        self.compose = compose
        self.selectRoom = selectRoom
        self.availableRooms = availableRooms
    }
}

/// The library: a type specimen book with eight pages.
///
/// Swiping sideways turns a page. While the page is being turned it lifts off
/// the sheet and the bench shows between two pages — the one moment in the
/// product where paper is handled rather than read.
public struct LibraryScreen: View {
    @StateObject private var model: LibraryModel
    @Environment(\.tr) private var tr
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var drag: CGFloat = 0
    @State private var markLift: CGFloat = 0

    private let actions: LibraryActions

    public init(store: TypviaStore, opening sort: TypeSort = .text, actions: LibraryActions = LibraryActions()) {
        _model = StateObject(wrappedValue: LibraryModel(store: store, opening: sort))
        self.actions = actions
    }

    public var body: some View {
        GeometryReader { proxy in
            VStack(alignment: .leading, spacing: 0) {
                header
                page
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    // Lifting the page reveals the bench it was lying on.
                    .background(isSecretChapter ? VaultRoom.base : Paper.base)
                    .offset(x: drag)
                    .animation(Beat.transition.enter(reduceMotion: reduceMotion), value: model.index)
                    .gesture(turn(width: proxy.size.width))
                footer
            }
            .background(isSecretChapter ? VaultRoom.carrier : Paper.carrier)
        }
        .room(model.chapter.style.accent)
        .onAppear { model.open(model.index) }
    }

    private var header: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(tr("Library", "资料库"))
                .typviaType(.sectionTitle)
                .foregroundStyle(isSecretChapter ? VaultRoom.ink : Paper.ink)
            Spacer(minLength: 0)
            Text(
                tr(
                    "Chapter \(model.index + 1) / \(Chapter.order.count)",
                    "章 \(model.index + 1) / \(Chapter.order.count)"
                )
            )
            .typviaType(.mono)
            .foregroundStyle(isSecretChapter ? VaultRoom.ink3 : Paper.ink3)
        }
        .padding(.horizontal, Tokens.Space.screenPadding)
        .padding(.top, LibraryMetrics.headerTop)
        .padding(.bottom, LibraryMetrics.headerBottom)
        .background(isSecretChapter ? VaultRoom.base : Paper.base)
    }

    @ViewBuilder
    private var page: some View {
        if isSecretChapter {
            // Not a page turn: the material itself changes. The secret chapter
            // is an ink room whatever the app's theme is.
            SecretChapter(chapter: model.chapter, openVault: actions.openVault)
                .transition(.opacity)
        } else {
            ScrollView(showsIndicators: false) {
                VStack(alignment: .leading, spacing: 0) {
                    ChapterHead(chapter: model.chapter)
                        // The mark arrives after the paper has landed.
                        .offset(y: markLift)
                        .padding(.bottom, LibraryMetrics.headBottom)
                    chapterBody
                    // The way to add one, in every chapter that is not empty.
                    // It used to be offered only on an empty chapter, so the
                    // moment a reader saved their first snippet of a kind, the
                    // room they browse that kind in stopped being a room they
                    // could add to.
                    if let compose = actions.compose, model.chapter.phase != .empty {
                        StickVerb(
                            title: tr("Write one", "写一枚"),
                            isPrimary: true,
                            action: compose
                        )
                        .padding(.top, LibraryMetrics.rowGap)
                    }
                }
                .padding(.horizontal, Tokens.Space.screenPadding)
                .padding(.bottom, Tokens.Space.section)
            }
        }
    }

    @ViewBuilder
    private var chapterBody: some View {
        switch model.chapter.phase {
        case .loading:
            ChapterSkeleton()
        case .empty:
            EmptyChapter(sort: model.chapter.sort, compose: actions.compose)
        case .unavailable:
            UnavailableChapter()
        case .listed, .locked:
            LazyVStack(alignment: .leading, spacing: LibraryMetrics.rowGap) {
                ForEach(model.chapter.groups) { group in
                    if let title = group.title {
                        GroupHeading(title: title)
                    }
                    ForEach(group.rows) { row in
                        ChapterRow(row: row, style: model.chapter.style, open: actions.open)
                            .onAppear {
                                // The last row coming into view is the request
                                // for the next screenful. A big chapter arrives
                                // a page at a time rather than all at once.
                                guard row.id == model.chapter.rows.last?.id else { return }
                                Task { await model.loadMore() }
                            }
                    }
                }
            }
        }
    }

    private var footer: some View {
        VStack(alignment: .leading, spacing: 0) {
            SortBar(current: model.sort, onInkRoom: isSecretChapter) { sort in
                guard let index = Chapter.order.firstIndex(of: sort) else { return }
                Haptic.light.play()
                model.open(index)
            }
            .padding(.horizontal, Tokens.Space.screenPadding)
            .padding(.top, LibraryMetrics.sortBarTop)
            Text(tr("Swipe to turn chapters", "横滑翻章"))
                .typviaType(.monoLabel)
                .foregroundStyle(isSecretChapter ? VaultRoom.ink3 : Paper.ink3)
                .padding(.horizontal, Tokens.Space.screenPadding)
                .padding(.top, LibraryMetrics.sortBarHintGap)
            RoomBar(
                current: .library,
                available: actions.availableRooms,
                onInkRoom: isSecretChapter,
                select: actions.selectRoom
            )
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(isSecretChapter ? VaultRoom.base : Paper.base)
    }

    private func turn(width: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: LibraryMetrics.turnMinimum)
            .onChanged { value in
                guard ChapterPaging.canBegin(atX: value.startLocation.x) else { return }
                drag = value.translation.width
            }
            .onEnded { value in
                guard ChapterPaging.canBegin(atX: value.startLocation.x) else { return }
                let step = ChapterPaging.step(forDrag: value.translation.width, width: width)
                withAnimation(Beat.transition.enter(reduceMotion: reduceMotion)) {
                    drag = 0
                }
                guard step != 0 else { return }
                Haptic.light.play()
                model.turn(by: step)
                liftMark()
            }
    }

    /// The turned-to chapter's mark rises in the closing stretch of the turn,
    /// the way a specimen page settles once the paper is already down.
    private func liftMark() {
        guard !reduceMotion else { return }
        let beat = Beat.transition.duration
        markLift = ChapterPaging.markLift
        withAnimation(
            Curve.enter(ChapterPaging.markLiftDuration(in: beat))
                .delay(ChapterPaging.markLiftDelay(in: beat))
        ) {
            markLift = 0
        }
    }

    private var isSecretChapter: Bool { model.chapter.sort == .secret }
}

/// Measurements this screen takes from its own frames.
enum LibraryMetrics {
    static let headerTop: CGFloat = 14
    static let headerBottom: CGFloat = 18
    static let headNameGap: CGFloat = 18
    static let headCountGap: CGFloat = 6
    static let headBottom: CGFloat = 30

    static let rowGap: CGFloat = 18
    static let rowInnerGap: CGFloat = 6
    static let rowTitleGap: CGFloat = 10
    static let rowPadding: CGFloat = 4
    static let groupHeadingTop: CGFloat = 12
    static let groupHeadingBottom: CGFloat = 2

    static let gridStep: CGFloat = 12
    static let gridOpacity = 0.5

    /// An empty chapter still exists; a loading one is on its way.
    static let emptyMarkOpacity = 0.28
    static let loadingMarkOpacity = 0.5

    static let sortBarGap: CGFloat = 4
    static let sortBarItem: CGFloat = 28
    static let sortBarMarker: CGFloat = 2
    static let sortBarTop: CGFloat = 10
    static let sortBarHintGap: CGFloat = 2

    static let turnMinimum: CGFloat = 18

    static let skeletonRowGap: CGFloat = 18
    static let skeletonTitleWidth: CGFloat = 132
    static let skeletonTitleHeight: CGFloat = 15
    static let skeletonBodyHeight: CGFloat = 13
    static let skeletonRadius: CGFloat = 2
    static let skeletonStagger = 0.060
    static let skeletonBreath = 1.400
    static let skeletonFloor = 0.12
    static let skeletonPeak = 0.26

    static let lockedTop: CGFloat = 30
    static let lockedRowGap: CGFloat = 14
    static let lockedBodyGap: CGFloat = 34
    static let lockedActionGap: CGFloat = 18

    static let emptyBodyGap: CGFloat = 14
    static let emptyActionsGap: CGFloat = 30
}

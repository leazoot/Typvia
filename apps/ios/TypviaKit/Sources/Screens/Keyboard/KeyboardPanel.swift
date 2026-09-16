// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// What the panel hands back to the host controller.
public struct KeyboardActions {
    /// Types a body into whatever field the reader is in.
    public var insert: ((String) -> Void)?
    public var undo: (() -> Void)?
    public var takeOutSecret: ((String) -> Void)?
    public var saveWhatIsTyped: (() -> Void)?
    public var switchKeyboard: (() -> Void)?

    public init(
        insert: ((String) -> Void)? = nil,
        undo: (() -> Void)? = nil,
        takeOutSecret: ((String) -> Void)? = nil,
        saveWhatIsTyped: (() -> Void)? = nil,
        switchKeyboard: (() -> Void)? = nil
    ) {
        self.insert = insert
        self.undo = undo
        self.takeOutSecret = takeOutSecret
        self.saveWhatIsTyped = saveWhatIsTyped
        self.switchKeyboard = switchKeyboard
    }
}

/// The composing bench: a borderless search row, the sorts, a run of tiles,
/// and the function row.
public struct KeyboardPanel: View {
    @Environment(\.tr) private var tr
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// How far the ink has opened, as a fraction of the bench.
    @State private var bloom: Double = 0

    private let phase: KeyboardPhase
    private let tiles: [KeyboardTile]
    private let reach: KeyboardReach
    private let actions: KeyboardActions
    @Binding private var query: String

    public init(
        phase: KeyboardPhase,
        tiles: [KeyboardTile],
        reach: KeyboardReach,
        query: Binding<String>,
        actions: KeyboardActions = KeyboardActions()
    ) {
        self.phase = phase
        self.tiles = tiles
        self.reach = reach
        _query = query
        self.actions = actions
    }

    public var body: some View {
        ZStack {
            bench
                .opacity(isSecret ? 0 : 1)
            if case let .secret(title) = phase {
                // The whole bench becomes an ink room: one thing at a time.
                // The ink arrives as a circle opening from the middle of the
                // bench rather than as a new screen — the room is the same
                // room, with the light changed.
                KeyboardSecret(title: title, takeOut: actions.takeOutSecret)
                    .background(VaultRoom.base)
                    .clipShape(InkBloom(extent: bloom))
                    .transition(.identity)
            }
        }
        .frame(height: KeyboardBand.total)
        .background(Paper.carrier)
        .room(.home)
        // A panel that is already in the ink room when it first appears has
        // not just opened — it is open. Growing the circle from nothing here
        // would draw a blank bench for a frame, which is what a render of this
        // state caught the first time.
        .onAppear { bloom = isSecret ? Tokens.Motion.inkBloomExtent : 0 }
        .onChange(of: isSecret) { becameSecret in
            guard becameSecret else {
                bloom = 0
                return
            }
            // Asked for less motion means no expanding circle at all: the room
            // is simply the other room, and the bench's own fade carries it.
            guard !reduceMotion else {
                bloom = Tokens.Motion.inkBloomExtent
                return
            }
            bloom = 0
            withAnimation(Curve.enter(Tokens.Motion.inkBloomDuration)) {
                bloom = Tokens.Motion.inkBloomExtent
            }
        }
    }

    private var isSecret: Bool {
        if case .secret = phase { return true }
        return false
    }

    private var bench: some View {
        VStack(spacing: 0) {
            KeyboardSearchRow(query: $query, phase: phase, reach: reach, matches: tiles.count)
                .frame(height: KeyboardBand.search)
            KeyboardSortBar(phase: phase)
                .frame(height: KeyboardBand.sorts)
            content
                .frame(height: KeyboardBand.tiles)
            KeyboardFunctionRow(switchKeyboard: actions.switchKeyboard)
                .frame(height: KeyboardBand.functions)
            // The home indicator's strip: left empty on purpose.
            Color.clear.frame(height: KeyboardBand.gutter)
        }
    }

    @ViewBuilder
    private var content: some View {
        switch phase {
        case let .noMatch(query):
            KeyboardNoMatch(query: query, save: actions.saveWhatIsTyped)
        case .limited:
            KeyboardNothingToRead()
        case let .inserted(title):
            VStack(spacing: 0) {
                InsertedNote(title: title, undo: actions.undo)
                tileRun
            }
        case .browsing, .filtering, .secret:
            tileRun
        }
    }

    private var tileRun: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: KeyboardMetrics.tileGap) {
                ForEach(tiles) { tile in
                    KeyboardTileView(tile: tile) {
                        if tile.isSecret {
                            actions.takeOutSecret?(tile.id)
                        } else {
                            actions.insert?(tile.id)
                        }
                    }
                }
            }
            .padding(.horizontal, KeyboardMetrics.inset)
            // Filtering moves tiles rather than replacing them: the ones that
            // survive a keystroke slide to their new places, so a reader who
            // was reaching for the second tile can see it travel instead of
            // finding something else where it was.
            .animation(Beat.state.enter(reduceMotion: reduceMotion), value: tiles)
        }
    }
}

/// The search row: a caret, what is typed, and a hairline. No border — the
/// most expensive thing in a keyboard is height, and a border does not earn it.
struct KeyboardSearchRow: View {
    @Environment(\.tr) private var tr

    @Binding var query: String
    let phase: KeyboardPhase
    let reach: KeyboardReach

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: KeyboardMetrics.rowGap) {
                Caret(height: KeyboardMetrics.caretHeight, behaviour: query.isEmpty ? .breathing : .blinking)
                Text(query.isEmpty ? placeholder : query)
                    .typviaType(.bodyS)
                    .foregroundStyle(query.isEmpty ? Paper.ink3 : Paper.ink)
                    .lineLimit(1)
                Spacer(minLength: 0)
                Text(count)
                    .typviaType(.mono)
                    .foregroundStyle(Paper.ink3)
            }
            .padding(.horizontal, KeyboardMetrics.inset)
            .frame(maxHeight: .infinity)
            Hairline()
        }
    }

    /// With nothing to read, "search the last 0" is a line that counts a thing
    /// that is not there. The plain wording is the honest one — there is
    /// nothing to narrow, and the row below says why.
    private var placeholder: String {
        if reach.total == 0 || reach.isComplete {
            return tr("Search snippets", "搜索片段")
        }
        return tr("Search the last \(reach.available)", "搜索最近 \(reach.available) 枚")
    }

    private var count: String {
        switch phase {
        case .filtering where matches > 0:
            return tr.counted(matches, "match", "matches", "\(matches) 枚匹配")
        default:
            return reach.total == 0 || reach.isComplete
                ? String(reach.total)
                : "\(reach.available) / \(reach.total)"
        }
    }

    /// Handed in by the panel. The row counts nothing itself — a view that
    /// counts is a view that can disagree with the list beside it.
    let matches: Int
}

/// The eight sorts as a filter strip. Kinds with no hit fade rather than
/// disappearing: a strip that reflows under a reader's thumb is a strip they
/// stop trusting.
struct KeyboardSortBar: View {
    @Environment(\.tr) private var tr

    let phase: KeyboardPhase

    var body: some View {
        HStack(spacing: KeyboardMetrics.sortGap) {
            Text(tr("All", "全部"))
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.ink2)
            ForEach(TypeSort.allCases, id: \.self) { sort in
                Text(sort.code)
                    .typviaType(.monoLabel)
                    .foregroundStyle(Paper.ink3)
                    .opacity(isFiltering ? KeyboardMetrics.dimmedSort : 1)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, KeyboardMetrics.inset)
        .accessibilityHidden(true)
    }

    private var isFiltering: Bool {
        if case .filtering = phase { return true }
        return false
    }
}

/// One tile on the bench.
struct KeyboardTileView: View {
    @Environment(\.tr) private var tr

    let tile: KeyboardTile
    let use: () -> Void

    var body: some View {
        Button(action: use) {
            PaperCard {
                VStack(alignment: .leading, spacing: KeyboardMetrics.tileInnerGap) {
                    if let sort = tile.sort {
                        TypeSortMark(
                            sort, size: .compact,
                            accessibilityLabel: sort.name(tr)
                        )
                    }
                    Text(tile.title)
                        .typviaType(.sectionTitle)
                        .foregroundStyle(Paper.ink)
                        .lineLimit(2)
                    trailing
                    Spacer(minLength: 0)
                }
                .padding(KeyboardMetrics.tilePadding)
                .frame(
                    width: tile.isLead ? KeyboardTile.leadWidth : KeyboardTile.width,
                    alignment: .topLeading
                )
            }
            // Just used: it steps back rather than vanishing, so the run does
            // not reshuffle under the thumb that just tapped it.
            .opacity(tile.isSpent ? KeyboardMetrics.spentTile : 1)
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private var trailing: some View {
        if tile.isSecret {
            // The dots are the title now; this line says what the tap will
            // ask for rather than repeating them.
            Text(tr("Needs verifying", "需验证"))
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
        } else if tile.isSpent {
            Text(tr("just used", "刚用过"))
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
        } else if let trigger = tile.trigger {
            Text(trigger)
                .typviaType(.mono)
                .foregroundStyle(Room.home.accent)
                .lineLimit(1)
        }
    }
}

/// Something was typed into the host app. The note lives four seconds, covers
/// nothing, and offers the one thing worth offering.
struct InsertedNote: View {
    @Environment(\.tr) private var tr

    let title: String
    let undo: (() -> Void)?

    /// How long the note stays before it goes.
    static let life: Double = 4

    var body: some View {
        HStack(spacing: KeyboardMetrics.rowGap) {
            Text(tr("Inserted \"\(title)\"", "已插入「\(title)」"))
                .typviaType(.mono)
                .foregroundStyle(Paper.ink2)
                .lineLimit(1)
            Spacer(minLength: 0)
            if let undo {
                Button(action: undo) {
                    Text(tr("Undo", "撤销"))
                        .typviaType(.mono)
                        .foregroundStyle(Room.home.accent)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, KeyboardMetrics.inset)
        .frame(height: KeyboardMetrics.noteHeight)
    }
}

/// A query with nothing behind it. There are 118 points to work with, so it is
/// a caret, a sentence and two words — and it is also a way in.
struct KeyboardNoMatch: View {
    @Environment(\.tr) private var tr

    let query: String
    let save: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: KeyboardMetrics.tileInnerGap) {
            Caret(height: KeyboardMetrics.caretHeight)
            Text(tr("Nothing is saved under that word yet.", "这个词还没有对应的片段。"))
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink)
                .fixedSize(horizontal: false, vertical: true)
            if let save {
                KeyboardVerb(title: tr("Save what is typed", "存为新片段"), action: save)
            }
        }
        .padding(.horizontal, KeyboardMetrics.inset)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// The extension has no document to read.
///
/// This state is reached in exactly one way: there was no snapshot at all —
/// either the app has never written one, or this process cannot see the shared
/// container. It used to tell the reader to turn on full access, which is
/// wrong twice over: this keyboard declares that it does not want full access,
/// so the switch is not even drawn in the system's settings, and the switch
/// was never what stood between it and the document anyway.
///
/// The two causes are not distinguishable from in here, so neither is named.
/// What is offered is the act that fixes the one the reader can fix, and it is
/// harmless for the other.
struct KeyboardNothingToRead: View {
    @Environment(\.tr) private var tr

    var body: some View {
        VStack(alignment: .leading, spacing: KeyboardMetrics.tileInnerGap) {
            // What is still true comes first: nothing has been lost.
            Text(KeyboardPanelCopy.nothingToRead(tr))
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink)
                .fixedSize(horizontal: false, vertical: true)
            Text(KeyboardPanelCopy.openTheAppOnce(tr))
                .typviaType(.caption)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, KeyboardMetrics.inset)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// A secret was chosen. The bench becomes an ink room, says how the text will
/// travel rather than that the reader is in danger, and shows nothing.
struct KeyboardSecret: View {
    @Environment(\.tr) private var tr

    let title: String
    let takeOut: ((String) -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: KeyboardMetrics.tileInnerGap) {
            TypeSortMark(.secret, size: .compact, on: .inkRoom, accessibilityLabel: tr("Secret", "密钥"))
            Text(tr("Take out \"\(title)\" with Face ID", "用面容 ID 取出「\(title)」"))
                .typviaType(.sectionTitle)
                .foregroundStyle(VaultRoom.ink)
                .fixedSize(horizontal: false, vertical: true)
            Text(
                tr(
                    "It goes straight into the field. It is not shown in the keyboard and it does not touch the clipboard.",
                    "它会直接落进输入框,不在键盘里显示,也不进剪贴板。"
                )
            )
            .typviaType(.caption)
            .foregroundStyle(VaultRoom.ink2)
            .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: KeyboardMetrics.rowGap) {
                Caret(height: KeyboardMetrics.caretHeight, color: VaultRoom.accent)
                Text(tr("Waiting for Face ID", "正在等待面容 ID"))
                    .typviaType(.mono)
                    .foregroundStyle(VaultRoom.ink3)
            }
        }
        .padding(KeyboardMetrics.inset)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(VaultRoom.base)
        .room(.vault)
    }
}

/// The function row: the globe the system requires, and the two shelves.
struct KeyboardFunctionRow: View {
    @Environment(\.tr) private var tr

    let switchKeyboard: (() -> Void)?

    var body: some View {
        HStack(spacing: KeyboardMetrics.rowGap) {
            if let switchKeyboard {
                Button(action: switchKeyboard) {
                    Text(tr("ABC", "ABC"))
                        .typviaType(.monoLabel)
                        .foregroundStyle(Paper.ink2)
                        .frame(minWidth: Tokens.Hit.minimum, minHeight: Tokens.Hit.minimum)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(tr("Switch keyboard", "切换键盘"))
            }
            Text(tr("Recent", "最近"))
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.ink2)
            Text(tr("Starred", "收藏"))
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.ink3)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, KeyboardMetrics.inset)
    }
}

/// A word in the panel. The keyboard keeps its own so that it links no part
/// of the app's screens: an extension under a memory ceiling should not carry
/// a library screen to draw one button.
struct KeyboardVerb: View {
    let title: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink)
                .frame(minHeight: Tokens.Hit.minimum)
        }
        .buttonStyle(.plain)
    }
}

enum KeyboardMetrics {
    /// The frames use 18 on Android and 26 on iOS from the same source; the
    /// keyboard takes the smaller, because 260 points is all there is.
    static let inset: CGFloat = 18
    static let rowGap: CGFloat = 10
    static let sortGap: CGFloat = 10
    static let tileGap: CGFloat = 10
    static let tilePadding: CGFloat = 12
    static let tileInnerGap: CGFloat = 6
    static let caretHeight: CGFloat = 18
    static let noteHeight: CGFloat = 28
    static let dimmedSort = 0.45
    static let spentTile = 0.55
}

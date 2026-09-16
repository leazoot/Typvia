// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// The four rooms, as words.
///
/// No icons, no tab bar chrome, no pill behind the current room: the room you
/// are in is the one with a caret in front of it. The bar sits on the sheet
/// with nothing under it, because the delivery's list of forbidden things
/// includes a bottom bar that covers content.
public struct RoomBar: View {
    /// The rooms that have a door. `ai` is not among them: it is a capability
    /// the other rooms use, not a place.
    public static let rooms: [Room] = [.home, .library, .vault, .settings]

    @Environment(\.tr) private var tr

    private let current: Room
    private let available: Set<Room>
    private let onInkRoom: Bool
    private let select: ((Room) -> Void)?

    /// - Parameter available: the rooms that have been built. The others are
    ///   still printed — the bar is the product's map and hiding a room would
    ///   misdraw it — but they are text rather than controls, because a word
    ///   that does nothing when tapped is worse than a word that is plainly
    ///   not a door yet.
    /// - Parameter onInkRoom: the bar sits on a dark room — the shut chapter,
    ///   and the vault when it arrives — and takes that room's colours.
    public init(
        current: Room,
        available: Set<Room> = [],
        onInkRoom: Bool = false,
        select: ((Room) -> Void)? = nil
    ) {
        self.current = current
        self.available = available
        self.onInkRoom = onInkRoom
        self.select = select
    }

    public var body: some View {
        HStack(spacing: 0) {
            ForEach(RoomBar.rooms, id: \.self) { room in
                entry(for: room)
            }
        }
        .padding(.top, Metrics.topPadding)
        .padding(.bottom, Metrics.bottomPadding)
        // The bar is a strip across the foot of the screen, not a row of words
        // pushed to one side. A hairline separates it from the page so it does
        // not read as more content.
        .frame(maxWidth: .infinity)
        .overlay(alignment: .top) { Hairline() }
    }

    @ViewBuilder
    private func entry(for room: Room) -> some View {
        let isCurrent = room == current
        let label = VStack(spacing: Metrics.iconGap) {
            RoomIcon(room: room, isCurrent: isCurrent)
                .foregroundStyle(ink(isCurrent: isCurrent))
            Text(name(of: room))
                .typviaType(.monoLabel)
                .foregroundStyle(ink(isCurrent: isCurrent))
        }
        // Each door takes an equal quarter and the platform's full minimum
        // height, so the whole column is the target rather than the word.
        .frame(maxWidth: .infinity, minHeight: Tokens.Hit.minimum)
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityLabel(name(of: room))
        .accessibilityAddTraits(isCurrent ? [.isSelected] : [])

        if let select, !isCurrent, available.contains(room) {
            Button { select(room) } label: { label }
                .buttonStyle(.plain)
        } else {
            label
        }
    }

    /// Which door you are standing in is said twice — the icon and its word
    /// both take full ink, the other three sit at meta. Colour alone never
    /// carries a meaning in this product, and here it does not have to: the
    /// weight of the type differs as well.
    private func ink(isCurrent: Bool) -> Color {
        if isCurrent {
            return onInkRoom ? VaultRoom.ink : Paper.ink
        }
        return onInkRoom ? VaultRoom.ink3 : Paper.ink3
    }

    private func name(of room: Room) -> String {
        switch room {
        case .home: tr("Home", "首页")
        case .library: tr("Library", "资料库")
        case .vault: tr("Vault", "保险库")
        case .settings: tr("Settings", "设置")
        case .ai: tr("AI", "AI")
        }
    }

    /// Taken from the frames, which draw this bar identically on every screen.
    private enum Metrics {
        static let iconGap: CGFloat = 5
        static let topPadding: CGFloat = 10
        static let bottomPadding: CGFloat = 4
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI
import UIKit

/// The vault: the one room whose material breaks from the rest of the app.
///
/// It is an ink room in either theme. Shut, it shows three things — a caret
/// breathing slowly, one sentence, and seven dots where a count would be. No
/// lock icon, no "your data is encrypted" sales pitch, and no hint of how much
/// is in here.
public struct VaultScreen: View {
    @StateObject private var model: VaultModel
    @Environment(\.tr) private var tr
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.scenePhase) private var scenePhase
    @State private var showsPasswordField = false
    /// How far the ink has opened. Full at rest — a room that is already open
    /// has not just opened, and starting it at nothing would draw a blank
    /// sheet for a frame.
    @State private var bloom = VaultMetrics.bloomExtent
    @State private var password = ""

    private let actions: VaultActions

    public init(store: TypviaStore, actions: VaultActions = VaultActions()) {
        _model = StateObject(wrappedValue: VaultModel(store: store))
        self.actions = actions
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            content
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .padding(.horizontal, Tokens.Space.screenPadding)
            RoomBar(
                current: .vault,
                available: actions.availableRooms,
                onInkRoom: true,
                select: actions.selectRoom
            )
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(VaultRoom.base)
        // The ink opens from the middle when the vault does — the one 320ms
        // moment in the product, and the only circular one. The number for it
        // has been sitting in this file since the room was built, unused: the
        // room simply appeared, which is what every other room does.
        .clipShape(InkBloom(extent: bloom))
        .room(.vault)
        .task { await model.load() }
        .onChange(of: model.phase) { phase in
            guard phase == .open else {
                // Shutting is not the bloom in reverse. The room is simply
                // shut again, and the design gives closing its own shorter
                // beat rather than a circle collapsing.
                bloom = VaultMetrics.bloomExtent
                return
            }
            guard !reduceMotion else { return }
            bloom = 0
            withAnimation(Curve.enter(Tokens.Motion.inkBloomDuration)) {
                bloom = VaultMetrics.bloomExtent
            }
        }
        // Leaving, or the app going away, shuts it. No transition: a vault
        // that closes gracefully is a vault that is briefly still open.
        .onDisappear { model.lockImmediately() }
        .onChange(of: scenePhase) { phase in
            if phase != .active { model.lockImmediately() }
        }
        // A screenshot is a copy of this room that outlives the session, in a
        // library any app with photo access can read. The system tells us
        // afterwards — the picture is already taken — so what this can do is
        // make it the last one: the room is shut by the time anyone looks back
        // at the screen.
        .onReceive(
            NotificationCenter.default.publisher(
                for: UIApplication.userDidTakeScreenshotNotification
            )
        ) { _ in
            model.lockImmediately()
        }
    }

    private var header: some View {
        Text(headline)
            .typviaType(.monoLabel)
            .foregroundStyle(VaultRoom.ink3)
            .padding(.horizontal, Tokens.Space.screenPadding)
            .padding(.top, VaultMetrics.headerTop)
            .padding(.bottom, VaultMetrics.headerBottom)
    }

    private var headline: String {
        switch model.phase {
        case .shut, .didNotOpen, .absent: tr("VAULT · SHUT", "保险库 · 已锁")
        case .opening: tr("VAULT · OPENING", "保险库 · 正在打开")
        case .open: tr("VAULT · OPEN", "保险库 · 已开")
        }
    }

    @ViewBuilder
    private var content: some View {
        switch model.phase {
        case .shut, .opening:
            ShutVault(
                isOpening: model.phase == .opening,
                showsPasswordField: $showsPasswordField,
                password: $password,
                openWithBiometrics: { Task { await model.openWithBiometrics() } },
                openWithPassword: { Task { await model.open(password: password); password = "" } }
            )
        case let .didNotOpen(door):
            DidNotOpen(
                door: door,
                refusal: model.refusal,
                showsPasswordField: $showsPasswordField,
                password: $password,
                retry: { Task { await model.openWithBiometrics() } },
                openWithPassword: { Task { await model.open(password: password); password = "" } }
            )
        case .open:
            ScrollView(showsIndicators: false) {
                OpenVault(entries: model.entries, clock: model.clock, open: actions.open)
            }
        case .absent:
            AbsentVault(putOneAway: actions.putOneAway)
        }
    }
}

/// What the vault room can hand back.
public struct VaultActions {
    public var open: ((String) -> Void)?
    /// Opens the editor on a secret. This is how a vault is made — there is no
    /// separate "create a vault" anywhere, because a vault with nothing in it
    /// is a password somebody set for no reason.
    public var putOneAway: (() -> Void)?
    public var selectRoom: ((Room) -> Void)?
    public var availableRooms: Set<Room>

    public init(
        open: ((String) -> Void)? = nil,
        putOneAway: (() -> Void)? = nil,
        selectRoom: ((Room) -> Void)? = nil,
        availableRooms: Set<Room> = []
    ) {
        self.open = open
        self.putOneAway = putOneAway
        self.selectRoom = selectRoom
        self.availableRooms = availableRooms
    }
}

enum VaultMetrics {
    static let headerTop: CGFloat = 14
    static let headerBottom: CGFloat = 30
    static let caretHeight: CGFloat = 52
    static let caretGap: CGFloat = 30
    static let lineGap: CGFloat = 18
    static let actionGap: CGFloat = 26
    static let actionRowGap: CGFloat = 20
    static let rowGap: CGFloat = 22
    static let rowInnerGap: CGFloat = 5
    static let listTop: CGFloat = 26
    static let clockLineHeight: CGFloat = 2
    static let clockGap: CGFloat = 10
    static let fieldGap: CGFloat = 12
    /// The one 320ms moment in the product, and the only circular bloom.
    static let bloomExtent = Tokens.Motion.inkBloomExtent
    static let openingDim = 0.5
}

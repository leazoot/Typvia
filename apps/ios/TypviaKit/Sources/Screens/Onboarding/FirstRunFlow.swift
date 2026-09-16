// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// The first launch: three screens, and every one of them can be left.
///
/// What is demonstrated is the product — a line typing itself out with the
/// trigger that would have produced it — rather than an illustration of one.
/// The way straight in is on every screen and is never dimmed: a first run
/// that cannot be left is a first run that gets force-quit.
public struct FirstRunFlow: View {
    @Environment(\.tr) private var tr
    @Environment(\.scenePhase) private var scenePhase

    @State private var page: FirstRun = .save
    @State private var install: KeyboardInstall = .notAdded

    private let finish: () -> Void
    private let openSystemSettings: () -> Void

    /// - Parameters:
    ///   - openSystemSettings: handed in rather than called from here, because
    ///     this framework is linked into the keyboard and the widget too, and
    ///     neither of them may open an app's settings.
    ///   - finish: the reader is through — by arriving at the end, or by
    ///     saying they would rather just start.
    public init(
        openSystemSettings: @escaping () -> Void,
        finish: @escaping () -> Void
    ) {
        self.openSystemSettings = openSystemSettings
        self.finish = finish
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            FirstRunScreen(
                page: page,
                next: advance,
                skip: finish,
                openSystemSettings: page == .keyboard ? openSystemSettings : nil,
                install: install
            )
            // The page grows downward from the top, as the frames have it: the
            // three screens hold different amounts, and centring each one
            // would move the headline every time the reader turns a page.
            Spacer(minLength: 0)
        }
        .padding(.horizontal, Tokens.Space.screenPadding)
        .padding(.top, FirstRunMetrics.top)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .background(Paper.base)
        .room(.home)
        // Swipe or tap, as the delivery has it. The threshold is generous:
        // this is a page turn, not a control.
        .gesture(
            DragGesture(minimumDistance: FirstRunMetrics.turnDistance)
                .onEnded { drag in
                    guard abs(drag.translation.width) > abs(drag.translation.height) else { return }
                    turn(back: drag.translation.width > 0)
                }
        )
        .onAppear { install = KeyboardInstall.current() }
        // The reader leaves for the system settings to add the keyboard, so
        // the only way to know whether they did is to look on the way back.
        .onChange(of: scenePhase) { phase in
            guard phase == .active else { return }
            install = KeyboardInstall.current()
        }
    }

    /// Moving on from the last screen is finishing. The keyboard screen has no
    /// "next" of its own — its verbs are the system settings and "later" — so
    /// this is reached by the two before it and by a swipe.
    private func advance() {
        guard let next = FirstRun(rawValue: page.rawValue + 1) else {
            finish()
            return
        }
        page = next
    }

    private func turn(back: Bool) {
        if back {
            guard let previous = FirstRun(rawValue: page.rawValue - 1) else { return }
            page = previous
        } else {
            advance()
        }
    }
}

extension KeyboardInstall {
    /// What the system's own list of keyboards says right now.
    ///
    /// Checked rather than assumed, and read again every time the app comes
    /// back: the reader adds the keyboard somewhere else entirely. A list that
    /// cannot be read reads as *not confirmed*, which is what the screen does
    /// with it — it adds a line when the keyboard is there and stays quiet
    /// otherwise, so an unreadable list costs a reassurance rather than
    /// producing a false one.
    public static func current(defaults: UserDefaults = .standard) -> KeyboardInstall {
        state(activeInputModes: defaults.array(forKey: installedKeyboardsKey) as? [String] ?? [])
    }

    /// Where the system records the keyboards this device has active.
    static let installedKeyboardsKey = "AppleKeyboards"
}

enum FirstRunMetrics {
    /// How far a finger travels before it counts as turning the page.
    static let turnDistance: CGFloat = 40
    static let top: CGFloat = 30
}

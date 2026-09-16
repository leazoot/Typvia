// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI
import UIKit

/// What one snippet's screen can hand back.
public struct DetailActions {
    public var close: (() -> Void)?
    public var insert: ((String) -> Void)?
    public var copy: ((String) -> Void)?

    public init(
        close: (() -> Void)? = nil,
        insert: ((String) -> Void)? = nil,
        copy: ((String) -> Void)? = nil
    ) {
        self.close = close
        self.insert = insert
        self.copy = copy
    }
}

/// A snippet, open. One canvas: the body is the subject and every tool is in
/// the composing stick along the bottom, where a thumb reaches.
///
/// The kind of snippet is said by the material the body sits on — a grid for
/// code, plain paper for prose, frosted for a secret — rather than by a label
/// repeating what the mark already says.
public struct DetailScreen: View {
    @StateObject private var model: DetailModel
    @Environment(\.tr) private var tr
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.scenePhase) private var scenePhase
    @FocusState private var isComposing: Bool
    @State private var isStickOpen = false
    @State private var impress = false

    private let actions: DetailActions

    public init(store: TypviaStore, snippetId: String, actions: DetailActions = DetailActions()) {
        _model = StateObject(wrappedValue: DetailModel(store: store, snippetId: snippetId))
        self.actions = actions
    }

    public init(
        store: TypviaStore,
        clipboard: String? = nil,
        sort: TypeSort? = nil,
        actions: DetailActions = DetailActions()
    ) {
        _model = StateObject(
            wrappedValue: DetailModel(store: store, clipboard: clipboard, sort: sort)
        )
        self.actions = actions
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            // The screen scrolls, so the screen is what brings the caret
            // back. The keyboard takes the bottom half and the composing
            // stick sits on top of that; without this the reader taps the
            // body, the sheet slides up, and what they type is behind the
            // keyboard until they scroll for it by hand.
            ScrollViewReader { scroll in
                ScrollView(showsIndicators: false) {
                    DetailBody(model: model, isComposing: $isComposing)
                    .id(DetailScreen.composerAnchor)
                    .padding(.horizontal, Tokens.Space.screenPadding)
                    .padding(.bottom, Tokens.Space.section)
                    // Saving presses the words into the paper: everything
                    // sinks two points and comes back.
                    .offset(y: impress ? DetailMetrics.impressDrop : 0)
                }
                .onChange(of: isComposing) { isFocused in
                    guard isFocused else { return }
                    // After the keyboard's own animation has taken the height,
                    // not before it: scrolling to a point that is about to
                    // move puts the caret back under the keyboard.
                    Task {
                        try? await Task.sleep(nanoseconds: DetailMetrics.keyboardSettleNanos)
                        withAnimation(Beat.state.enter(reduceMotion: reduceMotion)) {
                            scroll.scrollTo(DetailScreen.composerAnchor, anchor: .top)
                        }
                    }
                }
            }
            ComposingStick(
                model: model,
                isOpen: $isStickOpen,
                actions: actions,
                onSave: { Task { await saveWithImpress() } }
            )
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(model.isSecret ? VaultRoom.base : Paper.base)
        .room(model.isSecret ? .vault : .home)
        .swipeToGoBack(actions.close)
        .task {
            await model.load()
            await model.loadActions()
        }
        // A secret goes back under when the screen goes away, by the same path
        // the countdown uses.
        .onDisappear { model.hideSecret() }
        // The other two ways a secret outlives the moment it was taken out for.
        // The vault room has had both since it was built, and this is the room
        // the plaintext is actually on: leaving it out is covering the door and
        // not the window.
        .onChange(of: scenePhase) { phase in
            // The app switcher photographs what is on screen, so this has to
            // happen on the way out of active rather than on the way back.
            if phase != .active { model.hideSecret() }
        }
        // The system tells us afterwards — the picture is already taken — so
        // what this can do is make it the last one.
        .onReceive(
            NotificationCenter.default.publisher(
                for: UIApplication.userDidTakeScreenshotNotification
            )
        ) { _ in
            model.hideSecret()
        }
    }

    private var header: some View {
        HStack {
            Button { actions.close?() } label: {
                Text(tr("Back", "返回"))
                    .typviaType(.bodyS)
                    .foregroundStyle(model.isSecret ? VaultRoom.ink2 : Paper.ink2)
                    .frame(minHeight: Tokens.Hit.minimum)
            }
            .buttonStyle(.plain)
            .disabled(actions.close == nil)
            Spacer(minLength: 0)
            if let sort = model.draft.sort ?? model.snippet.flatMap({ TypeSort(coreType: $0.snippetType) }) {
                TypeSortMark(
                    sort,
                    size: .compact,
                    on: model.isSecret ? .inkRoom : .page,
                    accessibilityLabel: sort.name(tr)
                )
            }
        }
        .padding(.horizontal, Tokens.Space.screenPadding)
        .padding(.top, DetailMetrics.headerTop)
    }

    /// What the screen scrolls to when the body takes the caret.
    private static let composerAnchor = "composer"

    private func saveWithImpress() async {
        // Only a write is pressed into the paper. A draft that stopped at the
        // vault gate has not been written, and an impression there would be
        // the screen telling the reader something is filed when it is not.
        guard await model.save() else { return }
        isStickOpen = false
        Haptic.medium.play()
        guard !reduceMotion else { return }
        withAnimation(Curve.enter(DetailMetrics.impressDuration)) { impress = true }
        try? await Task.sleep(nanoseconds: UInt64(DetailMetrics.impressDuration * 1_000_000_000))
        withAnimation(Curve.enter(DetailMetrics.impressDuration)) { impress = false }
    }
}

/// Measurements this screen takes from its own frames.
enum DetailMetrics {
    static let headerTop: CGFloat = 14
    static let titleTop: CGFloat = 26
    static let titleGap: CGFloat = 8
    static let bodyTop: CGFloat = 26

    /// The words sink into the paper as they are saved, and come back.
    static let impressDrop: CGFloat = 2
    static let impressDuration = 0.280

    static let stickTop: CGFloat = 16
    static let stickGap: CGFloat = 26
    static let stickRowGap: CGFloat = 18
    static let stickHandleWidth: CGFloat = 34
    static let stickHandleHeight: CGFloat = 3
    static let stickFieldGap: CGFloat = 8
    static let markRowGap: CGFloat = 6

    static let blankPaddingX: CGFloat = 6
    static let blankLabelWidth: CGFloat = 78
    static let blankPaddingY: CGFloat = 2
    static let blankRadius: CGFloat = 4

    static let countdownHeight: CGFloat = 2
    static let outputLines = 4
    static let composerMinHeight: CGFloat = 180
    static let disabledOpacity = 0.4
    static let secretLineGap: CGFloat = 14

    /// How long the keyboard takes to claim its height. Waiting it out is the
    /// difference between scrolling to where the caret will be and scrolling
    /// to where it was.
    static let keyboardSettleNanos: UInt64 = 320_000_000

    /// UITextView's own text container insets, which a `TextEditor` inherits
    /// and offers no way to change. The hint has to sit on them or it will not
    /// line up with the text that replaces it.
    static let editorInsetX: CGFloat = 5
    static let editorInsetY: CGFloat = 8
    static let savedNoteGap: CGFloat = 10
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// Pulling the sheet down to get at the search position.
///
/// The home screen is one sheet of paper lying on the workshop's inked bench.
/// Dragging it down slides it off the bench far enough to see the bench, and
/// letting go puts the reader in the search field. It is the one gesture in
/// the product where something is moved rather than operated, and no other
/// screen has it, which is why it lives with this screen instead of in the
/// design system.
public enum PullToReveal {
    /// The paper resists: it travels a little over half as far as the finger.
    public static let damping: CGFloat = 0.55
    /// Past this much travel, letting go opens the search field.
    public static let threshold: CGFloat = 64
    /// How far the hint sits below the top edge, in the gap the pull opens.
    static let hintInset: CGFloat = 10
    /// The space the shelf measures its own top edge in.
    static let shelfSpace = "pull-to-reveal-shelf"

    /// Where the sheet sits for a given finger displacement. Upward drags move
    /// nothing: there is no bench above the paper.
    public static func travel(forDrag drag: CGFloat) -> CGFloat {
        max(0, drag) * damping
    }

    public static func isPastThreshold(_ travel: CGFloat) -> Bool {
        travel >= threshold
    }

    /// Whether a shelf sitting at `scrollOffset` hands the drag on to the pull.
    ///
    /// Only a shelf at its own top does. Anywhere else the same downward drag
    /// means "scroll back up", and taking it would move the paper and the
    /// shelf at once — the reader asked for one of them.
    public static func yieldsToPull(scrollOffset: CGFloat) -> Bool {
        scrollOffset >= -0.5
    }
}

/// How far the shelf has been scrolled, reported upward to the pull. Zero at
/// the top, negative below it; zero is also the default, so a screen with no
/// shelf at all (empty, loading) pulls freely.
private struct ShelfOffsetKey: PreferenceKey {
    static let defaultValue: CGFloat = 0

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = nextValue()
    }
}

private struct PullingSheetKey: EnvironmentKey {
    static let defaultValue = false
}

extension EnvironmentValues {
    /// True while the paper is off the bench. The shelf stops scrolling for as
    /// long as it is: one finger, one thing moving.
    var isPullingSheet: Bool {
        get { self[PullingSheetKey.self] }
        set { self[PullingSheetKey.self] = newValue }
    }
}

/// The scrolling area of a screen that can be pulled down.
///
/// A plain `ScrollView` swallows every downward drag before the pull ever sees
/// it, which is why this exists rather than a modifier: the measurement has to
/// go *inside* the scrolled content, and the scroll has to stand down while the
/// paper is moving.
public struct PullAwareScroll<Content: View>: View {
    @Environment(\.isPullingSheet) private var isPulling

    private let content: Content

    public init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    public var body: some View {
        ScrollView(showsIndicators: false) {
            content
                // An overlay, not a sibling: a zero-height probe in the layout
                // would still be a row, and this one must cost nothing.
                .overlay(alignment: .top) {
                    GeometryReader { proxy in
                        Color.clear.preference(
                            key: ShelfOffsetKey.self,
                            value: proxy.frame(in: .named(PullToReveal.shelfSpace)).minY
                        )
                    }
                    .frame(height: 0)
                }
        }
        .coordinateSpace(name: PullToReveal.shelfSpace)
        .scrollDisabled(isPulling)
    }
}

extension View {
    /// - Parameters:
    ///   - isEnabled: off while the reader is already in the search field.
    ///     The gesture's whole purpose is to get there.
    ///   - onReveal: called once, on release past the threshold.
    public func pullToReveal(
        isEnabled: Bool,
        @ViewBuilder hint: @escaping (Bool) -> some View,
        onReveal: @escaping () -> Void
    ) -> some View {
        modifier(PullToRevealModifier(isEnabled: isEnabled, hint: hint, onReveal: onReveal))
    }
}

private struct PullToRevealModifier<Hint: View>: ViewModifier {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var travel: CGFloat = 0
    @State private var hasPassedThreshold = false
    @State private var shelfYields = true

    let isEnabled: Bool
    @ViewBuilder let hint: (Bool) -> Hint
    let onReveal: () -> Void

    func body(content: Content) -> some View {
        ZStack(alignment: .top) {
            // The bench the paper is lying on, visible only while it is off it.
            Paper.carrier.ignoresSafeArea()
            hint(hasPassedThreshold)
                .padding(.top, PullToReveal.hintInset)
                .opacity(travel > 0 ? 1 : 0)
                .animation(Beat.state.enter(reduceMotion: reduceMotion), value: travel > 0)

            content
                .background(Paper.base)
                .offset(y: travel)
                .environment(\.isPullingSheet, travel > 0)
                .onPreferenceChange(ShelfOffsetKey.self) { offset in
                    shelfYields = PullToReveal.yieldsToPull(scrollOffset: offset)
                }
                // Simultaneous, not exclusive: the shelf keeps its own drag for
                // scrolling, and this one runs beside it only where the shelf
                // has nothing left to give.
                .simultaneousGesture(drag, including: isActive ? .all : .subviews)
        }
    }

    private var isActive: Bool { isEnabled && shelfYields }

    private var drag: some Gesture {
        DragGesture(minimumDistance: 12)
            .onChanged { value in
                travel = PullToReveal.travel(forDrag: value.translation.height)
                let passed = PullToReveal.isPastThreshold(travel)
                guard passed != hasPassedThreshold else { return }
                hasPassedThreshold = passed
                // Only on the way in: a haptic on the way back out would
                // report crossing a line the reader is leaving.
                if passed { Haptic.light.play() }
            }
            .onEnded { _ in
                let reveal = hasPassedThreshold
                hasPassedThreshold = false
                // The sheet goes back at exit speed and without elasticity,
                // whether it is being released or cancelled.
                withAnimation(Beat.transition.exit(reduceMotion: reduceMotion)) {
                    travel = 0
                }
                if reveal { onReveal() }
            }
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Design tokens for the keyboard panel and the widgets. The keyboard follows
// the SYSTEM appearance, never the host app: colors are dynamic on the system
// interface style and geometry is identical across themes. Mono text renders
// in the system monospaced face, the sanctioned fallback for the design's
// IBM Plex Mono (the extension bundles no fonts).

import UIKit

enum PanelTheme {
    // MARK: Colors

    /// Warm app paper; the widget background.
    static let paper = dynamic(light: 0xFBFAF8, dark: 0x141516)
    /// Keyboard panel surface.
    static let panel = dynamic(light: 0xF2F0EC, dark: 0x1A1B1D)
    /// Vault tab surface, one brightness step below the panel.
    /// The design has no dark keyboard-vault surface; the dark value applies
    /// the same one-step rule to the dark panel.
    static let vaultPanel = dynamic(light: 0xEDEBE7, dark: 0x151617)
    static let ink = dynamic(light: 0x17181B, dark: 0xEDEDEB)
    static let secondary = dynamic(light: 0x7E8085, dark: 0x95979B)
    /// Third text tier: placeholders, inactive tabs, hint copy.
    static let meta = dynamic(light: 0xA5A29B, dark: 0x6A6C70)
    /// Mono micro-labels: wordmark, result count, LOCKED tag, dot strings.
    static let rowMeta = dynamic(light: 0xB0ADA6, dark: 0x63656A)
    /// Row preview line and the ABC key.
    static let preview = dynamic(light: 0x8C8A85, dark: 0x8A8C90)
    /// Ghost glyphs: the ↵ insert mark and the folder chevron.
    static let ghost = dynamic(light: 0xC3C0B9, dark: 0x4E5054)
    static let accent = dynamic(light: 0x4C6E8F, dark: 0x7FA3C4)
    /// Dimmed brand caret when search is unavailable (locked vault header).
    static let caretDim = inkAlpha(light: 0.24, dark: 0.24)
    /// Header and tab bar rules.
    static let hairline = inkAlpha(light: 0.07, dark: 0.08)
    /// Row separators sit one step quieter than structural hairlines.
    static let separator = inkAlpha(light: 0.06, dark: 0.07)
    /// Circle outline of the vault locked figure.
    static let outline = inkAlpha(light: 0.20, dark: 0.20)
    /// Raised card of the hold-to-preview overlay: white in light, the first
    /// brightening step in dark (two-step-lift rule).
    static let lifted = dynamic(light: 0xFFFFFF, dark: 0x242525)
    /// Template preview body text; dark value mirrors the dark
    /// keyboard's long-form text tone.
    static let previewText = dynamic(light: 0x3D4045, dark: 0xB9BABC)
    /// Resting underline of a template field.
    static let fieldRule = inkAlpha(light: 0.14, dark: 0.14)
    /// Ink-filled primary action; dark inverts to bright-on-dark, following
    /// the send-button language.
    static let actionBg = dynamic(light: 0x17181B, dark: 0xEDEDEB)
    static let actionText = dynamic(light: 0xF2F0EC, dark: 0x0E0F10)

    // MARK: Metrics (390pt-wide export frame)

    static let panelHeight: CGFloat = 356
    static let headerHeight: CGFloat = 48
    static let rowHeight: CGFloat = 59
    static let tabBarHeight: CGFloat = 44
    /// Space the design reserves under the tab bar (home indicator zone).
    static let bottomInset: CGFloat = 26
    static let hInset: CGFloat = 22

    // MARK: Type

    static let searchPlaceholderFont = UIFont.systemFont(ofSize: 14)
    static let queryFont = UIFont.monospacedSystemFont(ofSize: 14, weight: .regular)
    static let headerTitleFont = UIFont.systemFont(ofSize: 14, weight: .medium)
    static let backChevronFont = UIFont.systemFont(ofSize: 17, weight: .light)
    static let wordmarkFont = UIFont.systemFont(ofSize: 9.5, weight: .medium)
    static let countFont = UIFont.monospacedSystemFont(ofSize: 10.5, weight: .regular)
    static let rowTitleFont = UIFont.systemFont(ofSize: 14.5, weight: .medium)
    static let rowPreviewFont = UIFont.systemFont(ofSize: 11.5)
    static let rowPreviewMonoFont = UIFont.monospacedSystemFont(ofSize: 11.5, weight: .regular)
    static let returnMarkFont = UIFont.monospacedSystemFont(ofSize: 12, weight: .regular)
    static let dotsFont = UIFont.monospacedSystemFont(ofSize: 12.5, weight: .regular)
    static let lockedTagFont = UIFont.monospacedSystemFont(ofSize: 10, weight: .regular)
    static let tabFont = UIFont.systemFont(ofSize: 11)
    static let tabActiveFont = UIFont.systemFont(ofSize: 11, weight: .medium)
    static let abcFont = UIFont.monospacedSystemFont(ofSize: 10.5, weight: .regular)
    static let hintFont = UIFont.systemFont(ofSize: 11)
    static let vaultTitleFont = UIFont.systemFont(ofSize: 16)
    static let vaultSubFont = UIFont.systemFont(ofSize: 12.5)
    static let previewBodyFont = UIFont.monospacedSystemFont(ofSize: 12, weight: .regular)
    static let capsFont = UIFont.monospacedSystemFont(ofSize: 9.5, weight: .regular)
    static let fieldValueFont = UIFont.systemFont(ofSize: 14)
    static let previewTextFont = UIFont.systemFont(ofSize: 13)
    static let insertLabelFont = UIFont.systemFont(ofSize: 14, weight: .medium)
    static let insertReturnFont = UIFont.monospacedSystemFont(ofSize: 11, weight: .regular)
    static let copyFont = UIFont.systemFont(ofSize: 13.5, weight: .medium)

    // MARK: Letter spacing (design em values × point size)

    static let rowTitleKern: CGFloat = -0.174
    static let dotsKern: CGFloat = 1.75
    static let lockedTagKern: CGFloat = 1.3
    static let wordmarkKern: CGFloat = 1.9
    static let vaultTitleKern: CGFloat = -0.256
    static let capsKern: CGFloat = 1.235

    private static func dynamic(light: Int, dark: Int) -> UIColor {
        UIColor { traits in
            traits.userInterfaceStyle == .dark ? rgb(dark) : rgb(light)
        }
    }

    /// Hairline family: ink at low alpha in light, bright ink in dark.
    private static func inkAlpha(light: CGFloat, dark: CGFloat) -> UIColor {
        UIColor { traits in
            traits.userInterfaceStyle == .dark
                ? rgb(0xEDEDEB).withAlphaComponent(dark)
                : rgb(0x17181B).withAlphaComponent(light)
        }
    }

    private static func rgb(_ hex: Int) -> UIColor {
        UIColor(
            red: CGFloat((hex >> 16) & 0xFF) / 255,
            green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255,
            alpha: 1)
    }
}

/// Presentation vocabulary: core snippet-type strings → two-letter mono
/// marks (mirror of packages/ui markForType; native layer cannot import the
/// TS module). Unknown values read as plain text.
enum TypeMark {
    private static let markByType: [String: String] = [
        "text": "TX", "markdown": "TX", "code": "CD", "command": "CM",
        "prompt": "PR", "template": "TP", "sensitive": "SC",
        "ai_action": "AI", "link": "LK", "temporary": "TX",
    ]

    /// Full words for accessibility: screen readers get the word, never the
    /// two letters.
    private static let wordByMark: [String: String] = [
        "TX": "text", "CD": "code", "CM": "command", "PR": "prompt",
        "TP": "template", "SC": "secret", "AI": "AI action", "LK": "link",
    ]

    static func mark(for snippetType: String) -> String {
        markByType[snippetType] ?? "TX"
    }

    static func word(for mark: String) -> String {
        wordByMark[mark] ?? "text"
    }
}

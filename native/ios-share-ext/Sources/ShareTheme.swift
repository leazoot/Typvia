// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Design tokens for the share save sheet. The design draws the light theme
// only; dark values follow the system dark palette (#141516 base,
// #EDEDEB/#95979B/#6A6C70 text, #7FA3C4 accent) with the sheet lifted one
// step above the base. Colors are dynamic on the system interface style;
// geometry is identical across themes.

import UIKit

enum ShareTheme {
    // MARK: Colors (design values / dark derivations)

    /// Backdrop over the host app: rgba(20,21,22,.34) in both themes.
    static let dim = UIColor(
        red: 0x14 / 255, green: 0x15 / 255, blue: 0x16 / 255, alpha: 0.34)
    /// Sheet plate #FBFAF8; dark is a lift above the #141516 app base.
    static let sheet = dynamic(light: 0xFBFAF8, dark: 0x1C1D1F)
    /// Shared-text box #F3F1EC; dark is one more lift above the sheet.
    static let sunken = dynamic(light: 0xF3F1EC, dark: 0x242526)
    static let ink = dynamic(light: 0x17181B, dark: 0xEDEDEB)
    /// Mono text inside the shared-text box (#2B2D31, softer than ink).
    static let codeInk = dynamic(light: 0x2B2D31, dark: 0xD9DAD8)
    static let secondary = dynamic(light: 0x7E8085, dark: 0x95979B)
    static let meta = dynamic(light: 0xA5A29B, dark: 0x6A6C70)
    /// Mono field labels ("TITLE") #B0ADA6, the warm meta family.
    static let metaMono = dynamic(light: 0xB0ADA6, dark: 0x757370)
    /// The uppercase sheet header "SAVE TO TYPVIA" #8C8A85.
    static let headerInk = dynamic(light: 0x8C8A85, dark: 0x7F8185)
    static let accent = dynamic(light: 0x4C6E8F, dark: 0x7FA3C4)
    /// Field underline rgba(23,24,27,.10); dark mirrors it on light ink.
    static let hairline = dynamic(
        light: 0x17181B, lightAlpha: 0.10, dark: 0xEDEDEB, darkAlpha: 0.12)
    /// Grabber rgba(23,24,27,.14).
    static let grabber = dynamic(
        light: 0x17181B, lightAlpha: 0.14, dark: 0xEDEDEB, darkAlpha: 0.16)
    /// Primary action plate: ink-filled in light, inverted in dark.
    static let buttonFill = dynamic(light: 0x17181B, dark: 0xEDEDEB)
    static let buttonLabel = dynamic(light: 0xFBFAF8, dark: 0x1C1D1F)
    /// Sheet shadow 0 -10px 44px rgba(20,21,22,.16); light-theme only in
    /// effect (dark expresses the float through the plate lift instead).
    static let shadowColor = UIColor(
        red: 0x14 / 255, green: 0x15 / 255, blue: 0x16 / 255, alpha: 1)
    static let shadowOpacity: Float = 0.16
    static let shadowOffset = CGSize(width: 0, height: -10)
    /// CALayer's shadowRadius is the CSS blur halved.
    static let shadowRadius: CGFloat = 22

    // MARK: Metrics (sheet geometry, points)

    static let sheetCorner: CGFloat = 26
    static let sidePad: CGFloat = 26
    static let topPad: CGFloat = 14
    static let grabberSize = CGSize(width: 34, height: 4)
    static let headerBarSize = CGSize(width: 2, height: 11)
    static let noticeBarWidth: CGFloat = 2
    static let previewCorner: CGFloat = 8
    static let previewPad: CGFloat = 16
    static let previewLineHeight: CGFloat = 22
    static let previewMaxHeight: CGFloat = 132
    static let fieldRuleHeight: CGFloat = 1
    static let buttonHeight: CGFloat = 52
    static let buttonCorner: CGFloat = 12
    static let disabledAlpha: CGFloat = 0.35

    // MARK: Type (mono is the system ui-monospace stack)

    static let headerFont = UIFont.systemFont(ofSize: 10.5, weight: .medium)
    /// letter-spacing .2em at 10.5pt.
    static let headerKern: CGFloat = 2.1
    static let previewFont = UIFont.monospacedSystemFont(ofSize: 13, weight: .regular)
    static let metaFont = UIFont.systemFont(ofSize: 12.5)
    static let fieldLabelFont = UIFont.monospacedSystemFont(ofSize: 10, weight: .regular)
    /// letter-spacing .13em at 10pt.
    static let fieldLabelKern: CGFloat = 1.3
    static let fieldFont = UIFont.systemFont(ofSize: 16.5)
    /// letter-spacing -.014em at 16.5pt.
    static let fieldKern: CGFloat = -0.23
    static let noticeFont = UIFont.systemFont(ofSize: 14.5)
    static let noticeDetailFont = UIFont.systemFont(ofSize: 13)
    static let buttonFont = UIFont.systemFont(ofSize: 15, weight: .medium)
    static let cancelFont = UIFont.systemFont(ofSize: 14)

    private static func dynamic(light: Int, dark: Int) -> UIColor {
        dynamic(light: light, lightAlpha: 1, dark: dark, darkAlpha: 1)
    }

    private static func dynamic(
        light: Int, lightAlpha: CGFloat, dark: Int, darkAlpha: CGFloat
    ) -> UIColor {
        UIColor { traits in
            traits.userInterfaceStyle == .dark
                ? rgb(dark, alpha: darkAlpha)
                : rgb(light, alpha: lightAlpha)
        }
    }

    private static func rgb(_ hex: Int, alpha: CGFloat) -> UIColor {
        UIColor(
            red: CGFloat((hex >> 16) & 0xFF) / 255,
            green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255,
            alpha: alpha)
    }
}

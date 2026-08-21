// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Design tokens for the share save sheet (same values as the iOS ShareTheme —
// separate modules cannot import each other). The design draws the light theme
// only; dark values follow the system dark palette (#141516 base,
// #EDEDEB/#95979B/#6A6C70 text, #7FA3C4 accent) with the sheet lifted one step
// above the base. Colors follow the SYSTEM appearance; geometry is identical
// across themes.

package dev.typvia.share

import android.content.Context
import android.content.res.Configuration
import android.util.TypedValue
import kotlin.math.roundToInt

class ShareTheme(context: Context) {
    private val dark =
        context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK ==
            Configuration.UI_MODE_NIGHT_YES
    private val metrics = context.resources.displayMetrics
    private val density = context.resources.displayMetrics.density

    // Colors (design values / dark derivations), ARGB.

    /** Sheet plate #FBFAF8; dark is a lift above the #141516 app base. */
    val sheet = pick(0xFBFAF8, 0x1C1D1F)
    /** Shared-text box #F3F1EC; dark is one more lift above the sheet. */
    val sunken = pick(0xF3F1EC, 0x242526)
    val ink = pick(0x17181B, 0xEDEDEB)
    /** Mono text inside the shared-text box (#2B2D31, softer than ink). */
    val codeInk = pick(0x2B2D31, 0xD9DAD8)
    val secondary = pick(0x7E8085, 0x95979B)
    val meta = pick(0xA5A29B, 0x6A6C70)
    /** Mono field labels ("TITLE") #B0ADA6, the warm meta family. */
    val metaMono = pick(0xB0ADA6, 0x757370)
    /** The uppercase sheet header "SAVE TO TYPVIA" #8C8A85. */
    val headerInk = pick(0x8C8A85, 0x7F8185)
    val accent = pick(0x4C6E8F, 0x7FA3C4)
    /** Field underline rgba(23,24,27,.10); dark mirrors it on light ink. */
    val hairline = pickAlpha(0.10f, 0x17181B, 0.12f, 0xEDEDEB)
    /** Grabber rgba(23,24,27,.14). */
    val grabber = pickAlpha(0.14f, 0x17181B, 0.16f, 0xEDEDEB)
    /** Primary action plate: ink-filled in light, inverted in dark. */
    val buttonFill = pick(0x17181B, 0xEDEDEB)
    val buttonLabel = pick(0xFBFAF8, 0x1C1D1F)

    // Metrics in px (sheet geometry).

    val sheetCorner = dp(26f).toFloat()
    val sidePad = dp(26f)
    val topPad = dp(14f)
    val grabberWidth = dp(34f)
    val grabberHeight = dp(4f)
    val headerBarWidth = dp(2f)
    val headerBarHeight = dp(11f)
    val noticeBarWidth = dp(2f)
    val noticeBarHeight = dp(34f)
    val previewCorner = dp(8f).toFloat()
    val previewPad = dp(16f)
    val previewMaxHeight = dp(132f)
    val ruleHeight = dp(1f)
    val buttonHeight = dp(52f)
    val buttonCorner = dp(12f).toFloat()
    val minTapTarget = dp(44f)

    // Vertical rhythm (top margins between the sheet's blocks).

    val afterGrabber = dp(24f)
    val afterHeader = dp(22f)
    val afterPreview = dp(12f)
    val afterMeta = dp(18f)
    val afterFieldLabel = dp(10f)
    val afterField = dp(11f)
    val blockGap = dp(26f)
    val headerGap = dp(8f)
    val noticeGap = dp(9f)
    val noticeLineGap = dp(3f)
    val actionGap = dp(18f)
    /** Actions-to-sheet-bottom distance when no inset pushes further. */
    val sheetBottomResting = dp(40f)
    /** Clearance kept above the navigation/IME inset when it is larger. */
    val sheetBottomAboveInset = dp(18f)

    // Type sizes in sp (setTextSize COMPLEX_UNIT_SP call sites) and
    // letter-spacing in em (TextView.setLetterSpacing units).

    val headerSp = 10.5f
    val headerTracking = 0.2f
    val previewSp = 13f
    /** The mono box's fixed line height (13px/1.7 in the mock), px. */
    val previewLineHeight = sp(22f)
    val metaSp = 12.5f
    val fieldLabelSp = 10f
    val fieldLabelTracking = 0.13f
    val fieldSp = 16.5f
    val fieldTracking = -0.014f
    val noticeSp = 14.5f
    val noticeDetailSp = 13f
    val buttonSp = 15f
    val cancelSp = 14f

    val disabledAlpha = 0.35f

    fun dp(value: Float): Int = (value * density).roundToInt().coerceAtLeast(1)

    private fun sp(value: Float): Int =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, value, metrics)
            .roundToInt()
            .coerceAtLeast(1)

    private fun pick(light: Long, dark: Long): Int {
        val rgb = if (this.dark) dark else light
        return (0xFF000000L or rgb).toInt()
    }

    private fun pickAlpha(
        lightAlpha: Float,
        light: Long,
        darkAlpha: Float,
        dark: Long,
    ): Int {
        val alpha = if (this.dark) darkAlpha else lightAlpha
        val rgb = if (this.dark) dark else light
        return ((alpha * 255f).roundToInt().toLong() shl 24 or rgb).toInt()
    }
}

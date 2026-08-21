// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Design tokens for the keyboard panel. Pigment lives in res/values
// (+values-night) so the system appearance resolves it; this class carries
// the resolved colors plus the panel's metric and type scale.

package dev.typvia.ime

import android.content.Context
import kotlin.math.roundToInt

class PanelTheme(context: Context) {
    private val density = context.resources.displayMetrics.density

    // Colors, resolved through the resource system (values / values-night).
    val panelBg = context.getColor(R.color.tv_ime_panel)
    val vaultBg = context.getColor(R.color.tv_ime_panel_vault)
    val ink = context.getColor(R.color.tv_ime_ink)
    val secondary = context.getColor(R.color.tv_ime_secondary)
    val meta = context.getColor(R.color.tv_ime_meta)
    val micro = context.getColor(R.color.tv_ime_micro)
    val ghost = context.getColor(R.color.tv_ime_ghost)
    val accent = context.getColor(R.color.tv_ime_accent)
    val body = context.getColor(R.color.tv_ime_body)
    val outline = context.getColor(R.color.tv_ime_outline)
    val hairline = context.getColor(R.color.tv_ime_hairline)
    val divider = context.getColor(R.color.tv_ime_divider)
    val stroke = context.getColor(R.color.tv_ime_stroke)
    val caretDim = context.getColor(R.color.tv_ime_caret_dim)

    // Metrics in px (mock px read as dp; the 26px gesture strip under the
    // mock's footer belongs to the system, not the panel).
    val panelHeight = dp(330f)
    val barHeight = dp(52f)
    val headerHeight = dp(48f)
    val rowMinHeight = dp(58f)
    val footerHeight = dp(44f)
    val hInset = dp(22f)
    val stripRadius = dp(10f).toFloat()

    // Type sizes in sp (setTextSize COMPLEX_UNIT_SP call sites).
    val searchSp = 14f
    val rowTitleSp = 14.5f
    val rowSubSp = 11.5f
    val ghostSp = 12f
    val tabSp = 11f
    val countSp = 10.5f
    val wordmarkSp = 9.5f
    val lockedSp = 9.5f
    val stateMainSp = 14f
    val stateSubSp = 12.5f
    val stripSp = 13f
    val abcSp = 10.5f
    val capsLabelSp = 9.5f
    val fieldValueSp = 14f
    val previewSp = 13f
    val vaultMainSp = 16f
    val vaultSubSp = 12.5f
    val buttonSp = 14f

    fun dp(value: Float): Int = (value * density).roundToInt()
}

/**
 * Presentation vocabulary: core snippet-type strings mapped to the two-letter
 * marks of the design system (mirror of packages/ui markForType and the iOS
 * TypeMark; the native layer cannot import the TS module). The keyboard rows
 * no longer draw the mark, but screen readers still get the full word.
 */
object TypeMark {
    private val markByType = mapOf(
        "text" to "TX", "markdown" to "TX", "code" to "CD", "command" to "CM",
        "prompt" to "PR", "template" to "TP", "sensitive" to "SC",
        "ai_action" to "AI", "link" to "LK", "temporary" to "TX",
    )

    /** Full words for accessibility: screen readers get the word, never the
     * two letters. */
    private val wordByMark = mapOf(
        "TX" to "text", "CD" to "code", "CM" to "command", "PR" to "prompt",
        "TP" to "template", "SC" to "secret", "AI" to "AI action", "LK" to "link",
    )

    fun mark(snippetType: String): String = markByType[snippetType] ?: "TX"

    fun word(mark: String): String = wordByMark[mark] ?: "text"
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// The panel's resolved paint and measurements.
//
// Every value here comes from the design system the app is built from rather
// than from a copy of it: this panel used to carry its own colour resources,
// which meant the keyboard's paper could drift from the app's paper without
// anything failing. One palette, resolved once per input session.

package dev.typvia.ime

import android.content.Context
import android.content.res.Configuration
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.unit.Dp
import dev.typvia.mobile.ui.KeyboardBand
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.UiLanguage
import dev.typvia.mobile.ui.UiPreferenceFile
import dev.typvia.mobile.ui.VaultRoom
import kotlin.math.roundToInt

class PanelTheme(context: Context) {
    private val density = context.resources.displayMetrics.density

    /**
     * What the reader chose in the app, if they chose anything.
     *
     * This panel is a different process from the app and cannot be told, so it
     * reads the file the app leaves for it. Where there is nothing to read,
     * every value below falls back to the phone's own answer — which is what
     * this panel used to do unconditionally, and the reason a reader who set
     * the app to Chinese still got an English keyboard.
     */
    val chosen = UiPreferenceFile.read(context.dataDir)

    private val night = chosen.appearance.isDark(
        (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
            Configuration.UI_MODE_NIGHT_YES,
    )

    /** Which of the two languages this panel renders. Never both at once. */
    val language: UiLanguage = chosen.language.resolved(
        (0 until context.resources.configuration.locales.size()).map {
            context.resources.configuration.locales[it].toLanguageTag()
        },
    )

    val paper = swatch(Tokens.Swatch.paperLight, Tokens.Swatch.paperDark)
    val carrier = swatch(Tokens.Swatch.carrierLight, Tokens.Swatch.carrierDark)
    val ink = swatch(Tokens.Swatch.inkLight, Tokens.Swatch.inkDark)
    val ink2 = swatch(Tokens.Swatch.ink2Light, Tokens.Swatch.ink2Dark)
    val ink3 = swatch(Tokens.Swatch.ink3Light, Tokens.Swatch.ink3Dark)

    /** The keyboard is the home room's tool, and wears its accent. */
    val accent = swatch(Tokens.Swatch.homeLight, Tokens.Swatch.homeDark)

    /** The ink room is ink in either theme — the material is the point. */
    val vaultPaper = VaultRoom.base.toArgb()
    val vaultInk = VaultRoom.ink.toArgb()
    val vaultInk2 = VaultRoom.ink2.toArgb()
    val vaultAccent = VaultRoom.accent.toArgb()

    val hairline = alpha(
        ink,
        if (night) Tokens.Line.HAIRLINE_OPACITY_DARK else Tokens.Line.HAIRLINE_OPACITY_LIGHT,
    )

    // Measurements. The four bands and the gutter under them are the
    // delivery's; nothing here is a number this file thought of.
    val searchBand = px(KeyboardBand.search)
    val sortsBand = px(KeyboardBand.sorts)
    val tilesBand = px(KeyboardBand.tiles)
    val functionsBand = px(KeyboardBand.functions)
    val panelHeight = px(KeyboardBand.total)

    val screenPadding = px(Tokens.Space.screenPadding)
    val tileWidth = px(Tokens.Viewport.tileWidth)
    val tileLeadWidth = px(Tokens.Viewport.tileLeadWidth)
    val tileRadius = dp(Tokens.Radius.card.value).toFloat()
    val markRadius = dp(Tokens.Radius.sort.value).toFloat()
    val hairlineWidth = maxOf(1, px(Tokens.Line.hairlineWidth))
    val caretWidth = px(Tokens.Line.searchHeight) * 2

    // The type ladder, in sp, taken from the same table the app reads.
    val titleSp = Tokens.Type.bodyS.value
    val bodySp = Tokens.Type.caption.value
    val monoSp = Tokens.Type.monoLabel.value
    val markSp = Tokens.Type.sortMark.value

    /**
     * How faint a sort mark goes when nothing behind it matches. It stays on
     * the band at this opacity rather than leaving it.
     */
    val dimmedAlpha = 0.45f

    /** A spent tile steps back to here; it does not disappear. */
    val spentAlpha = 0.55f

    fun dp(value: Float): Int = (value * density).roundToInt()

    private fun px(value: Dp): Int = dp(value.value)

    private fun swatch(light: androidx.compose.ui.graphics.Color, dark: androidx.compose.ui.graphics.Color): Int =
        (if (night) dark else light).toArgb()

    private fun alpha(color: Int, fraction: Float): Int =
        (color and 0x00FFFFFF) or ((fraction * 255).roundToInt() shl 24)
}

/**
 * Presentation vocabulary: core snippet-type strings mapped to the two-letter
 * marks of the design system (mirror of packages/ui markForType and the iOS
 * TypeMark; the native layer cannot import the TS module).
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

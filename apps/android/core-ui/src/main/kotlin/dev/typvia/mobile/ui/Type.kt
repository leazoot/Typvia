// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/**
 * The type ladder, and the one place the two platforms genuinely differ in
 * type: what weight Chinese is set at.
 *
 * Latin keeps its semibold. Chinese on Android falls to Source Han Sans, which
 * at the same nominal weight is heavier than PingFang — a screen set the iOS
 * way comes out black. The delivery's instruction is exact: headings drop one
 * step (600 → 500), body stays where it is.
 */
enum class TypviaType {
    Display,
    Title1,
    Title2,
    Heading,
    Body,
    BodyS,
    SectionTitle,
    Caption,
    Mono,

    /**
     * A string the reader has to compare against another screen, character by
     * character. Large enough to read out loud, small enough that the whole of
     * it fits — the delivery drew this rung at 52pt for a six-character check
     * and the core hands over four groups of five, which at that size no phone
     * can set. Tracked wide, because the comparison is per character.
     */
    Code,
    SearchInput,
    MonoLabel,
    SortMark,
    ;

    private val size: TextUnit
        get() = when (this) {
            Display -> Tokens.Type.display
            Title1 -> Tokens.Type.title1
            Title2 -> Tokens.Type.title2
            Heading -> Tokens.Type.heading
            Body -> Tokens.Type.body
            BodyS -> Tokens.Type.bodyS
            SectionTitle -> Tokens.Type.sectionTitle
            Caption -> Tokens.Type.caption
            Mono -> Tokens.Type.mono
            Code -> Tokens.Type.code
            SearchInput -> Tokens.Type.searchInput
            MonoLabel -> Tokens.Type.monoLabel
            SortMark -> Tokens.Type.sortMark
        }

    private val lineHeightRatio: Float
        get() = when (this) {
            Display -> 1.05f
            Title1 -> 1.15f
            Title2 -> 1.25f
            Heading -> 1.40f
            Body -> 1.55f
            BodyS -> 1.60f
            SectionTitle -> 1.35f
            Caption -> 1.60f
            Mono -> 1.70f
            Code -> 1.20f
            SearchInput -> 1.20f
            MonoLabel -> 1.30f
            SortMark -> 1.20f
        }

    private val tracking: Float
        get() = when (this) {
            Display, SearchInput -> -0.02f
            Mono -> 0.02f
            Code -> 0.06f
            MonoLabel -> 0.20f
            else -> 0f
        }

    private val isMono: Boolean
        get() = this == Mono || this == MonoLabel || this == SortMark ||
            this == SearchInput || this == Code

    /**
     * The weight this level is set at, in the given language.
     *
     * @param language what is actually being rendered. The product renders one
     *   language at a time, so this is a property of the screen rather than of
     *   the string.
     */
    fun weight(language: UiLanguage): FontWeight = when (this) {
        Display, Title1, Title2, Heading, SectionTitle ->
            if (language == UiLanguage.Zh) FontWeight.Medium else FontWeight.SemiBold
        SortMark -> FontWeight.Bold
        else -> FontWeight.Normal
    }

    fun style(language: UiLanguage): TextStyle = TextStyle(
        fontSize = size,
        lineHeight = (size.value * lineHeightRatio).sp,
        fontWeight = weight(language),
        letterSpacing = tracking.em,
        fontFamily = if (isMono) FontFamily.Monospace else FontFamily.SansSerif,
    )
}

/** Which language the interface is rendering. Never both at once. */
enum class UiLanguage {
    En,
    Zh,
    ;

    companion object {
        /**
         * Resolves the system's preference to one of the two. Anything that is
         * not Chinese renders in English — the product has two languages and
         * picking the nearer of them is better than mixing.
         */
        fun resolve(tags: List<String>): UiLanguage =
            if (tags.firstOrNull()?.lowercase()?.startsWith("zh") == true) Zh else En
    }
}

/**
 * A count and the noun it counts, with English's plural decided in one place.
 *
 * English has a plural and Chinese does not, so the Chinese half is passed
 * whole and only the English half branches. Written out at each call site the
 * branch gets forgotten — "1 pieces", "1 minutes", "1 devices" — which is the
 * kind of mistake a reader reads as carelessness about everything else.
 *
 * Sentences whose verb also agrees ("1 change is queued") branch at their call
 * site instead: this decides a noun, not a grammar.
 *
 * The count is unsigned because the core's counts are, and Kotlin's number
 * types share no supertype that could answer "is this one" for all of them.
 * Call sites holding a signed size widen it rather than this narrowing theirs.
 */
fun Translator.counted(count: ULong, one: String, many: String, zh: String): String =
    this(if (count == 1uL) "$count $one" else "$count $many", zh)

/** How many pieces, in the four rooms that print a piece count. */
fun Translator.pieces(count: Int): String =
    counted(count.toULong(), "piece", "pieces", "$count 枚")

/**
 * Draws [content] at the size the design fixed, ignoring the reader's font
 * scale.
 *
 * Only for glyphs whose geometry cannot give: the two-letter kind mark is a
 * pair of letters inside a square of a fixed side, and **a mark that outgrew
 * its box would stop being a mark** — at the largest system size the letters
 * cross the 1.5dp stroke and, on the filled one, get cut off by its edge.
 *
 * Prose is never wrapped in this. The title beside a mark carries the reader's
 * chosen size; taking that away from running text would be refusing the
 * setting rather than laying out for it.
 */
@Composable
fun FixedTypeScale(content: @Composable () -> Unit) {
    val density = LocalDensity.current
    CompositionLocalProvider(
        LocalDensity provides Density(density.density, fontScale = 1f),
        content = content,
    )
}

/**
 * One string, in the language being rendered.
 *
 * Copy is written as an inline pair at the point it is used, and exactly one
 * half of it is ever rendered. A screen that renders both is the mixed-language
 * screen the delivery forbids.
 */
class Translator(val language: UiLanguage) {
    operator fun invoke(en: String, zh: String): String = when (language) {
        UiLanguage.En -> en
        UiLanguage.Zh -> zh
    }
}

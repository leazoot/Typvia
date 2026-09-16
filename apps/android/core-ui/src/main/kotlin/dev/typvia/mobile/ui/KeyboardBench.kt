// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * The keyboard is a composing bench, and on this platform it is 252dp tall.
 *
 * Everything here is measured against that height, because height is the most
 * expensive thing a keyboard has. The eight points iOS spends on the home
 * indicator go somewhere else here: the system draws its own navigation bar
 * under the keyboard, and the function row moves up rather than being drawn
 * beneath something that will be drawn over.
 */
object KeyboardBand {
    val search: Dp = 44.dp
    val sorts: Dp = 36.dp
    val tiles: Dp = 118.dp
    val functions: Dp = 44.dp

    /** What the panel occupies, and what the service asks the system for. */
    val total: Dp = Tokens.Viewport.imeHeight

    val bands: Dp get() = search + sorts + tiles + functions

    /**
     * What is left over for the system's own bar. It is left alone rather than
     * stretched into: a function row under the navigation bar is a function
     * row people mis-tap.
     */
    val gutter: Dp get() = total - bands
}

/** What the panel is doing. */
sealed interface KeyboardPhase {
    /** The bench at rest: recent tiles, all kinds. */
    data object Browsing : KeyboardPhase

    /** A trigger is being typed; the tiles are filtered. */
    data class Filtering(val query: String) : KeyboardPhase

    /** Something was just typed into the host app, and can be undone. */
    data class Inserted(val title: String) : KeyboardPhase

    /** A secret was chosen: the whole panel becomes an ink room. */
    data class Secret(val title: String) : KeyboardPhase

    /** A query with nothing behind it. The empty state is also a way in. */
    data class NoMatch(val query: String) : KeyboardPhase

    /**
     * There is no library to reach: no snapshot has been written yet, the
     * bytes could not be read, or the library is genuinely empty. One phase
     * covers all three because the reader can do the same one thing about
     * each of them, and because "I could not read it" must never be drawn as
     * "you have nothing".
     */
    data object Unreachable : KeyboardPhase
}

/**
 * Typing into someone else's text field, one character at a time, because that
 * is what the product does — it does not paste.
 *
 * The arithmetic is the other platform's, to the millisecond: the cap is on
 * the whole insertion rather than on each character, so a longer body types
 * faster instead of taking longer, and past the point where the motion could
 * be seen it simply lands.
 */
object TypeIn {
    /** The shortest gap worth making, in milliseconds. */
    const val SHORTEST_INTERVAL_MS = 4.0

    sealed interface Plan {
        /** All at once — the right answer for a long body, not a fallback. */
        data object AtOnce : Plan

        data class Character(val intervalMs: Double) : Plan
    }

    fun durationMs(characterCount: Int): Double = minOf(
        characterCount.toDouble() * Tokens.Motion.TYPE_IN_PER_CHARACTER,
        Tokens.Motion.TYPE_IN_CAP.toDouble(),
    )

    fun plan(characterCount: Int): Plan {
        if (characterCount <= 1) return Plan.AtOnce
        val interval = durationMs(characterCount) / characterCount
        return if (interval < SHORTEST_INTERVAL_MS) Plan.AtOnce else Plan.Character(interval)
    }
}

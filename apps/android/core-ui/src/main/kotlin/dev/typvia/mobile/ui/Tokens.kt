// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Every number this interface uses, transcribed from the same delivery the iOS
 * one was transcribed from.
 *
 * The delivery's Android page is explicit about what changes and what does
 * not: three papers, every colour, the type ladder, the corner radii, the two
 * elevations, the rule budget, the caret shapes, the eight sort marks, the
 * copy, the beats and the reduced-motion plan are **identical**. What moves is
 * the margin (26 → 20, because 360 has thirty fewer points to spend), the tile
 * widths, the IME height, and the weight Chinese is set at. Those four live
 * here as their own values rather than as edits to the shared ones, so a
 * reader can see exactly where the two platforms part.
 *
 * Screens never read this. They read [Paper], [Room], [TypviaType] and the
 * components — a number that is not in the delivery is not invented at a call
 * site.
 */
object Tokens {
    /** Raw sRGB values. What is a background and what is an accent is decided by [Paper]. */
    object Swatch {
        val paperLight = Color(0xFFFBFAF7)
        val paperDark = Color(0xFF14130F)
        val carrierLight = Color(0xFFF3F0E9)
        val carrierDark = Color(0xFF1E1C16)
        val inkLight = Color(0xFF1A1917)
        val inkDark = Color(0xFFEDE6D6)
        val ink2Light = Color(0xFF544F45)
        val ink2Dark = Color(0xFFB3AB99)
        val ink3Light = Color(0xFF6F695D)
        val ink3Dark = Color(0xFF8F887A)

        val homeLight = Color(0xFF31506F)
        val homeDark = Color(0xFF7FA3C9)
        val libraryLight = Color(0xFF8E651F)
        val libraryDark = Color(0xFFC9974A)
        val vaultLight = Color(0xFF2C4A3B)
        val vaultDark = Color(0xFF6FA98B)
        val settingsLight = Color(0xFF46443F)
        val settingsDark = Color(0xFF9A968C)
        val aiLight = Color(0xFFA0543A)
        val aiDark = Color(0xFFD08462)
    }

    object Space {
        /**
         * 26 on iOS, 20 here. The delivery takes the thirty points 360 does not
         * have out of the margin and out of nothing else — no font size and no
         * level of the hierarchy moves.
         */
        val screenPadding = 20.dp
        val row = 15.dp
        val group = 30.dp
        val section = 40.dp
    }

    object Radius {
        val sort = 3.dp
        val control = 8.dp
        val field = 10.dp
        val card = 12.dp
        val sheet = 22.dp
    }

    object Line {
        /** One device pixel at 2x is the thinnest a rule can be and still be a rule. */
        val hairlineWidth = 0.5.dp
        const val HAIRLINE_OPACITY_LIGHT = 0.08f
        const val HAIRLINE_OPACITY_DARK = 0.10f
        val hairlineInset = 24.dp

        /**
         * Not enforceable by a component — a screen with four rules compiles.
         * It is a review criterion, recorded here so the number has one home.
         */
        const val MAX_RULES_PER_SCREEN = 3

        val searchHeight = 1.dp
        const val SEARCH_REST_OPACITY = 0.20f
        const val SEARCH_FOCUS_OPACITY = 0.45f
    }

    /**
     * The beats, transcribed whole.
     *
     * Most of these are not referenced yet, and that is a statement about this
     * platform rather than about the sheet: **the interface here has no motion
     * at all so far** — no page turns, no cross-fades, no ink opening. The
     * numbers are kept because they are the delivery's, and because a sheet
     * with holes in it invites somebody to invent the missing value at a call
     * site. What is missing is the implementation, and it is registered as
     * such rather than hidden by deleting the evidence.
     */
    object Motion {
        const val TAP = 80
        const val STATE = 160
        const val TRANSITION = 240
        const val UNLOCK = 320

        /** Everything leaves faster than it arrived, and without elasticity. */
        const val EXIT_FACTOR = 0.7f

        /**
         * ease-out-quint, as the delivery's Android page gives it:
         * `PathInterpolator(.23, 1, .32, 1)` — the same four numbers iOS uses.
         */
        val enterCurve = floatArrayOf(0.23f, 1f, 0.32f, 1f)

        const val TYPE_IN_PER_CHARACTER = 26
        const val TYPE_IN_CAP = 240
        /**
         * The bloom's own duration, and the same 320ms as [UNLOCK] on purpose:
         * the delivery names it twice because it is the vault's beat *and* the
         * one flourish. The room animates through `Beat.Unlock`, so this is
         * the number's second name rather than a second number.
         */
        const val INK_BLOOM_DURATION = 320
        const val INK_BLOOM_EXTENT = 0.82f

        const val BREATHE_DURATION = 1600
        const val BREATHE_VAULT_DURATION = 2400

        /** Reduced motion replaces every animation with this cross-fade. */
        const val REDUCED_CROSS_FADE = 120
    }

    object Hit {
        /**
         * 48dp, not 44. The platform's own floor is higher than iOS's, and the
         * delivery's tile geometry is sized to clear it.
         */
        val minimum = 48.dp
    }

    object Viewport {
        /**
         * 252dp against iOS's 260. The eight points go to the system's own
         * navigation bar: the function row moves up rather than being drawn
         * underneath something the system will draw over.
         */
        val imeHeight = 252.dp

        /** Tile widths: 134 → 126 and 190 → 146, still showing two and a half. */
        val tileWidth = 126.dp
        val tileLeadWidth = 146.dp

        /** Gesture navigation leaves this much at the bottom of a full page. */
        val bottomSafeArea = 16.dp

        /**
         * The system's back gesture owns this much of each edge, so a page that
         * turns on a horizontal swipe must not start counting until past it.
         *
         * Unused so far, and that is a statement about this platform: **the
         * chapters do not turn on a swipe here yet**. The number is kept
         * because the day they do, the edge is not a thing to guess at.
         */
        val backGestureInset = 20.dp
    }

    object Type {
        val display = 44.sp
        val title1 = 34.sp
        val title2 = 30.sp
        val heading = 22.sp
        val body = 17.sp
        val bodyS = 15.sp
        val sectionTitle = 15.sp
        val caption = 13.sp
        val mono = 12.sp
        val code = 26.sp
        val searchInput = 26.sp
        val monoLabel = 11.sp
        val sortMark = 11.sp
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import android.provider.Settings
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import kotlin.math.max
import kotlin.math.sqrt

/**
 * The beats, as animations.
 *
 * The motif is typing: arrivals are derived from it, and departures are the
 * same motion at seven tenths of the duration and never elastic — leaving is
 * not an event the reader needs to watch.
 *
 * Everything here collapses to one short cross-fade when the reader has asked
 * for less motion. That is not a courtesy setting to honour where convenient:
 * a page that still slides after the request is a page that ignored it.
 */
enum class Beat(val durationMs: Int) {
    /** A tap landing: a selection, a copy, a key press. */
    Tap(Tokens.Motion.TAP),

    /** A state changing in place: a filter, a disclosure. */
    State(Tokens.Motion.STATE),

    /** A page arriving. */
    Transition(Tokens.Motion.TRANSITION),

    /** The vault opening, ink blooming. */
    Unlock(Tokens.Motion.UNLOCK),
    ;

    fun enter(reduceMotion: Boolean): FiniteAnimationSpec<Float> = when {
        reduceMotion -> tween(Tokens.Motion.REDUCED_CROSS_FADE)
        else -> tween(durationMs, easing = Curve.enter)
    }

    /** Leaving is the same shape, faster. */
    fun exit(reduceMotion: Boolean): FiniteAnimationSpec<Float> = when {
        reduceMotion -> tween(Tokens.Motion.REDUCED_CROSS_FADE)
        else -> tween((durationMs * Tokens.Motion.EXIT_FACTOR).toInt(), easing = Curve.enter)
    }
}

/** The one curve: ease-out-quint, from the delivery's own four numbers. */
object Curve {
    val enter: Easing = androidx.compose.animation.core.CubicBezierEasing(
        Tokens.Motion.enterCurve[0],
        Tokens.Motion.enterCurve[1],
        Tokens.Motion.enterCurve[2],
        Tokens.Motion.enterCurve[3],
    )
}

/**
 * Whether the reader has asked the system for less motion.
 *
 * Read from the system rather than from a switch of our own: somebody who has
 * turned animations off once should not have to find the setting again in
 * every app. A scale of zero is the platform's way of saying so.
 */
@Composable
@ReadOnlyComposable
fun systemReduceMotion(): Boolean {
    val resolver = LocalContext.current.contentResolver
    return Settings.Global.getFloat(
        resolver,
        Settings.Global.ANIMATOR_DURATION_SCALE,
        1f,
    ) == 0f
}

/**
 * Ink opening from the middle of a surface.
 *
 * The vault's one flourish, and the only 320ms in the product: the room does
 * not appear, it is opened. The same shape the other platform draws, taking
 * the same extent — a fraction of the surface's half-diagonal, so the circle
 * reaches the corners at the delivery's 0.82 rather than at some number that
 * happened to look right on one screen size.
 *
 * At rest — extent 0 — nothing is drawn, so a caller that forgets to animate
 * shows an empty room rather than a covered one. Callers start it open when
 * motion is reduced.
 */
class InkBloom(private val extent: Float) : Shape {
    override fun createOutline(
        size: Size,
        layoutDirection: LayoutDirection,
        density: Density,
    ): Outline {
        val reference = sqrt(size.width * size.width + size.height * size.height) / sqrt(2f)
        val radius = max(reference * extent, 0f)
        return Outline.Generic(
            Path().apply {
                addOval(
                    androidx.compose.ui.geometry.Rect(
                        center = Offset(size.width / 2f, size.height / 2f),
                        radius = radius,
                    ),
                )
            },
        )
    }
}

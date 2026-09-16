// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** The translator in force. One language is rendered; this says which. */
val LocalTranslator = compositionLocalOf { Translator(UiLanguage.En) }

/** Whether the reader has asked for less motion. */
val LocalReduceMotion = compositionLocalOf { false }

/**
 * The language and the appearance the whole tree is drawn in.
 *
 * Appearance defaults to the phone's own, so a surface with no reader to ask —
 * the keyboard resolves its own paint outside Compose — carries on as it did.
 */
@Composable
fun TypviaTheme(
    translator: Translator,
    appearance: UiPreference.Appearance = UiPreference.Appearance.System,
    reduceMotion: Boolean = false,
    content: @Composable () -> Unit,
) {
    CompositionLocalProvider(
        LocalTranslator provides translator,
        LocalAppearance provides appearance,
        LocalReduceMotion provides reduceMotion,
        content = content,
    )
}

/**
 * The caret: the product's one mark, and its only mascot.
 *
 * It has four characters and this is the resting one — a slow breath that
 * bottoms out well above nothing, because a caret that vanishes reads as a bug
 * rather than as waiting.
 */
@Composable
fun Caret(
    height: Dp,
    modifier: Modifier = Modifier,
    color: Color? = null,
    capped: Boolean = false,
    breathing: Boolean = true,
    vaultPace: Boolean = false,
) {
    val room = LocalRoom.current
    val stroke = color ?: room.accent
    val width = CaretShape.width(height)
    val reduceMotion = LocalReduceMotion.current

    val alpha = if (breathing && !reduceMotion) {
        val transition = rememberInfiniteTransition(label = "caret")
        transition.animateFloat(
            initialValue = 1f,
            targetValue = CaretShape.BREATHE_FLOOR,
            animationSpec = infiniteRepeatable(
                animation = tween(
                    if (vaultPace) Tokens.Motion.BREATHE_VAULT_DURATION else Tokens.Motion.BREATHE_DURATION,
                ),
                repeatMode = RepeatMode.Reverse,
            ),
            label = "breath",
        ).value
    } else {
        1f
    }

    Canvas(modifier = modifier.size(width = width, height = height)) {
        val strokeWidth = width.toPx()
        drawRect(
            color = stroke.copy(alpha = stroke.alpha * alpha),
            topLeft = Offset((size.width - strokeWidth) / 2f, 0f),
            size = Size(strokeWidth, size.height),
        )
        if (!capped) return@Canvas
        // The typewriter's crossbars, derived from the width rather than from
        // a table of their own.
        val overhang = strokeWidth * CaretShape.CAP_OVERHANG_RATIO
        val thickness = strokeWidth * CaretShape.CAP_THICKNESS_RATIO
        val barWidth = strokeWidth + overhang * 2
        val x = (size.width - barWidth) / 2f
        drawRect(
            color = stroke.copy(alpha = stroke.alpha * alpha),
            topLeft = Offset(x, 0f),
            size = Size(barWidth, thickness),
        )
        drawRect(
            color = stroke.copy(alpha = stroke.alpha * alpha),
            topLeft = Offset(x, size.height - thickness),
            size = Size(barWidth, thickness),
        )
    }
}

/**
 * A caret is specified by its height alone; its width follows.
 *
 * The delivery hand-tunes the width per frame, so the ladder here is what it
 * actually draws rather than a formula invented to fit.
 */
object CaretShape {
    const val BREATHE_FLOOR = 0.12f
    const val CAP_OVERHANG_RATIO = 2f
    const val CAP_THICKNESS_RATIO = 2f / 3f

    fun width(height: Dp): Dp = when {
        height < 20.dp -> 2.dp
        height < 34.dp -> 3.dp
        height < 56.dp -> 4.dp
        else -> 5.dp
    }
}

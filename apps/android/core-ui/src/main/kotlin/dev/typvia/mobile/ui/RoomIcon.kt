// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp

/**
 * The four doors, drawn.
 *
 * The delivery forbids icons in navigation and the bar was four words for that
 * reason. The reader who uses this product asked for the ordinary four-icon bar
 * instead, on the grounds that a word-only bar left them unable to tell what on
 * any screen was a control — a deliberate departure from the delivery, decided
 * by them and recorded as such.
 *
 * What the departure does *not* do is bring in an icon library. These are drawn
 * to the delivery's own icon rules — a 20-unit grid, a 1.5dp stroke that does
 * not scale with the glyph — so the bar reads as part of this product. Each
 * shape is the room's own subject rather than a metaphor, and iOS draws the
 * same four from the same numbers.
 */
@Composable
fun RoomIcon(room: Room, tint: Color, modifier: Modifier = Modifier) {
    Canvas(modifier = modifier.size(RoomIconMetrics.side)) {
        val unit = size.width / RoomIconMetrics.GRID
        val stroke = Stroke(width = RoomIconMetrics.stroke.toPx(), cap = StrokeCap.Round)
        fun line(x1: Float, y1: Float, x2: Float, y2: Float) = drawLine(
            color = tint,
            start = Offset(x1 * unit, y1 * unit),
            end = Offset(x2 * unit, y2 * unit),
            strokeWidth = stroke.width,
            cap = StrokeCap.Round,
        )
        fun box(x: Float, y: Float, w: Float, h: Float) = drawRoundRect(
            color = tint,
            topLeft = Offset(x * unit, y * unit),
            size = Size(w * unit, h * unit),
            cornerRadius = androidx.compose.ui.geometry.CornerRadius(2f * unit),
            style = stroke,
        )
        when (room) {
            // Home is where you type: the caret itself, with its cap.
            Room.Home -> {
                line(10f, 3f, 10f, 17f)
                line(6.5f, 3f, 13.5f, 3f)
                line(6.5f, 17f, 13.5f, 17f)
            }
            // The library: sheets, seen edge on.
            Room.Library -> {
                box(3f, 3f, 14f, 10f)
                line(5.5f, 16f, 14.5f, 16f)
            }
            // A plate with a keyhole. Not a padlock — this product does not
            // draw the reader a lock and tell them to feel safe.
            Room.Vault -> {
                box(3.5f, 3.5f, 13f, 13f)
                drawCircle(
                    color = tint,
                    radius = 1.75f * unit,
                    center = Offset(10f * unit, 9f * unit),
                    style = stroke,
                )
                line(10f, 10.75f, 10f, 13.5f)
            }
            // Three rules, the shortest last — the contents page's own shape,
            // not a cog.
            Room.Settings, Room.Ai -> {
                line(3f, 5f, 17f, 5f)
                line(3f, 10f, 13f, 10f)
                line(3f, 15f, 9f, 15f)
            }
        }
    }
}

object RoomIconMetrics {
    /** The delivery's icon grid. */
    const val GRID = 20f
    val side = 22.dp

    /** One weight at every size, as the delivery's icon rules require. */
    val stroke = 1.5.dp
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * The four doors, as words.
 *
 * No icons: the navigation is set in the same type as everything else, and
 * where the reader is standing is said by a caret beside the word rather than
 * by a colour or a filled shape. A room with no screen behind it is not
 * printed — a word in this bar is never a dead end.
 */
@Composable
fun RoomBar(
    current: Room,
    available: Set<Room>,
    modifier: Modifier = Modifier,
    onInkRoom: Boolean = false,
    select: ((Room) -> Unit)? = null,
) {
    val tr = LocalTranslator.current
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(if (onInkRoom) VaultRoom.base else Paper.base),
    ) {
        // A rule holds the bar off the page, so it does not read as more
        // content.
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(Tokens.Line.hairlineWidth)
                .background(Paper.rule),
        )
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(
                    top = RoomBarMetrics.top,
                    // Gesture navigation leaves a strip at the bottom, and the
                    // delivery keeps the paper above it rather than under it.
                    bottom = Tokens.Viewport.bottomSafeArea,
                ),
        ) {
            for (room in RoomBarOrder.rooms) {
                val isHere = room == current
                val isOpen = room in available
                val ink = when {
                    onInkRoom && isHere -> VaultRoom.ink
                    onInkRoom -> VaultRoom.ink3
                    isHere -> Paper.ink
                    else -> Paper.ink3
                }
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(RoomBarMetrics.iconGap),
                    modifier = Modifier
                        // An equal quarter each, and the whole column is the
                        // target rather than the word on it.
                        .weight(1f)
                        .heightIn(min = Tokens.Hit.minimum)
                        .clickable(enabled = select != null && !isHere && isOpen) {
                            select?.invoke(room)
                        }
                        .semantics(mergeDescendants = true) { selected = isHere },
                ) {
                    RoomIcon(room = room, tint = ink)
                    Text(
                        text = room.title(tr),
                        style = TypviaType.MonoLabel.style(tr.language),
                        color = ink,
                        textAlign = TextAlign.Center,
                    )
                }
            }
        }
    }
}


/** The order the doors are printed in — the reading order of the product. */
object RoomBarOrder {
    val rooms = listOf(Room.Home, Room.Library, Room.Vault, Room.Settings)
}

fun Room.title(tr: Translator): String = when (this) {
    Room.Home -> tr("Home", "首页")
    Room.Library -> tr("Library", "资料库")
    Room.Vault -> tr("Vault", "保险库")
    Room.Settings -> tr("Settings", "设置")
    Room.Ai -> tr("AI", "AI")
}

object RoomBarMetrics {
    /** Above the row of doors; the strip's own breathing room. */
    val top = 10.dp

    /** Between a door's mark and its word. */
    val iconGap = 4.dp

}

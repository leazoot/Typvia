// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.pieces
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.TypviaType
import uniffi.typvia_mobile_ffi.Snippet
import uniffi.typvia_mobile_ffi.trashRetentionDays

/**
 * The recycle bin: a quiet room, not a warning.
 *
 * What is in here is on its way out but is not gone, and the screen says both
 * halves plainly. The period it states is the core's own — read across the
 * bridge rather than written down here, because a screen that keeps its own
 * copy of a deadline will one day state one nobody is enforcing.
 */
@Composable
fun TrashScreen(
    rows: List<Snippet>?,
    onRestore: ((String) -> Unit)?,
    onEmpty: (() -> Unit)? = null,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
    var confirming by remember { mutableStateOf(false) }
    val days = trashRetentionDays().toInt()

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Paper.base)
            .verticalScroll(rememberScrollState())
            .padding(horizontal = Tokens.Space.screenPadding),
    ) {
        Text(
            text = tr("Back", "返回"),
            style = TypviaType.BodyS.style(tr.language),
            color = Paper.ink2,
            modifier = Modifier
                .padding(top = 14.dp, bottom = TrashMetrics.headGap)
                .clickable(onClick = onClose),
        )
        Text(
            text = tr("The bin", "回收站"),
            style = TypviaType.Title2.style(tr.language),
            color = Paper.ink,
        )
        Text(
            text = tr(
                "Anything here can be brought back for $days days, and then it goes.",
                "这里的东西 $days 天内随时能拿回来,之后就没了。",
            ),
            style = TypviaType.BodyS.style(tr.language),
            color = Paper.ink2,
            modifier = Modifier.padding(top = TrashMetrics.lineGap),
        )
        when {
            rows == null -> Text(
                text = tr("reading…", "正在读…"),
                style = TypviaType.Mono.style(tr.language),
                color = Paper.ink3,
                modifier = Modifier.padding(top = TrashMetrics.rowGap),
            )
            rows.isEmpty() -> Text(
                // Empty is the good outcome here, and it is said as one.
                text = tr("Nothing has been thrown away.", "没有扔掉过东西。"),
                style = TypviaType.BodyS.style(tr.language),
                color = Paper.ink3,
                modifier = Modifier.padding(top = TrashMetrics.rowGap),
            )
            else -> {
                if (onEmpty != null) {
                    if (!confirming) {
                        Text(
                            // The real number, in the verb itself: "empty the
                            // bin" hides how much is about to go.
                            text = tr(
                                "Destroy ${rows.size} for good",
                                "彻底删掉这 ${rows.size} 枚",
                            ),
                            style = TypviaType.BodyS.style(tr.language),
                            color = Paper.attention,
                            modifier = Modifier
                                .padding(top = TrashMetrics.rowGap)
                                .clickable { confirming = true },
                        )
                    } else {
                        Column(modifier = Modifier.padding(top = TrashMetrics.rowGap)) {
                            Box(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(Tokens.Line.hairlineWidth)
                                    .background(Paper.attention),
                            )
                            Text(
                                // The one place in the product where something
                                // really does go: said plainly, once.
                                text = tr(
                                    "${tr.pieces(rows.size)} will be gone. This one cannot be undone.",
                                    "这 ${rows.size} 枚会消失,这一步没法撤销。",
                                ),
                                style = TypviaType.BodyS.style(tr.language),
                                color = Paper.ink2,
                                modifier = Modifier.padding(top = TrashMetrics.lineGap),
                            )
                            Row(
                                modifier = Modifier.padding(top = TrashMetrics.lineGap),
                                horizontalArrangement = Arrangement.spacedBy(TrashMetrics.gap),
                            ) {
                                Text(
                                    text = tr("Destroy them", "删掉"),
                                    style = TypviaType.SectionTitle.style(tr.language),
                                    color = Paper.attention,
                                    modifier = Modifier.clickable {
                                        confirming = false
                                        onEmpty()
                                    },
                                )
                                Text(
                                    text = tr("Keep them", "留着"),
                                    style = TypviaType.BodyS.style(tr.language),
                                    color = Paper.ink2,
                                    modifier = Modifier.clickable { confirming = false },
                                )
                            }
                        }
                    }
                }
                for (row in rows) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = TrashMetrics.rowGap),
                    horizontalArrangement = Arrangement.spacedBy(TrashMetrics.gap),
                ) {
                    Text(
                        text = row.title,
                        style = TypviaType.SectionTitle.style(tr.language),
                        color = Paper.ink,
                        modifier = Modifier.weight(1f),
                    )
                    if (onRestore != null) {
                        Text(
                            text = tr("Bring it back", "拿回来"),
                            style = TypviaType.BodyS.style(tr.language),
                            color = Paper.ink,
                            modifier = Modifier.clickable { onRestore(row.id) },
                        )
                    }
                }
                }
            }
        }
    }
}

object TrashMetrics {
    val headGap = 26.dp
    val lineGap = 12.dp
    val rowGap = 18.dp
    val gap = 14.dp
}

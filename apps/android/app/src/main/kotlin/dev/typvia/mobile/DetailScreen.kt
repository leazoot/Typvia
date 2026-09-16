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
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
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
import dev.typvia.mobile.ui.BodyMask
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Room
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.TypeSort
import dev.typvia.mobile.ui.TypviaType
import dev.typvia.mobile.ui.VaultRoom
import uniffi.typvia_mobile_ffi.Snippet

/**
 * How often it has been used, in the meta line.
 *
 * English wants "once" where the count is one, and the sentence carries a verb
 * as well as a noun, so it branches here rather than through the shared noun
 * helper.
 */
private fun usageWord(count: ULong, tr: Translator): String =
    if (count == 1uL) tr("used once", "1 次") else tr("used $count times", "$count 次")

/**
 * One snippet, open.
 *
 * The body is the subject; the verbs sit under it. A secret is not shown here
 * at all — this platform has no vault flow yet, and a screen that offered to
 * reveal one and then could not would be worse than a screen that says what it
 * is and stops.
 */
@Composable
fun DetailScreen(
    snippet: Snippet,
    onCopy: ((String) -> Unit)?,
    onEdit: (() -> Unit)?,
    onTrash: (() -> Unit)?,
    onClose: () -> Unit,
    /** Takes the words out of the vault. Null when this one is not a secret. */
    onTakeOut: (() -> Unit)? = null,
    /** What was taken out, for as long as it is shown. */
    takenOut: String? = null,
    /** Moves an ordinary snippet into the vault. Null when it is already in. */
    onPutInVault: (() -> Unit)? = null,
    /**
     * What may be asked of a model about this snippet.
     *
     * A slot rather than a set of parameters, so this page stays free of the
     * store. It is composed below the verbs — that is, **after** the point a
     * secret's page has already returned — which is what keeps a secret out of
     * it: a rule that holds by position needs nobody to remember it.
     */
    aiStrip: (@Composable () -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    var confirming by remember { mutableStateOf(false) }
    val tr = LocalTranslator.current
    val sort = TypeSort.ofCoreType(snippet.snippetType)
    val isSecret = snippet.securityLevel != "normal"

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
                .padding(top = 14.dp, bottom = DetailMetrics.titleTop)
                .clickable(onClick = onClose),
        )
        Text(
            text = snippet.title,
            style = TypviaType.Title2.style(tr.language),
            color = Paper.ink,
        )
        Text(
            text = listOfNotNull(
                sort?.name(tr),
                if (isSecret) tr("Vault", "保险库") else usageWord(snippet.usageCount, tr),
            ).joinToString(" · "),
            style = TypviaType.Mono.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = DetailMetrics.metaTop),
        )
        snippet.trigger?.let { trigger ->
            Text(
                text = trigger,
                style = TypviaType.Mono.style(tr.language),
                color = Room.Home.accent,
                modifier = Modifier.padding(top = DetailMetrics.metaTop),
            )
        }
        val body = snippet.body
        if (isSecret || body == null) {
            // The row carries no body for a secret: it is fetched on the one
            // act that asks for it, shown for as long as that act lasts, and
            // is not kept by this page afterwards.
            Text(
                text = takenOut ?: BodyMask.SHORT,
                style = TypviaType.Mono.style(tr.language),
                color = if (takenOut != null) Paper.ink else Paper.ink3,
                modifier = Modifier.padding(top = DetailMetrics.bodyTop),
            )
            if (takenOut == null) {
                Text(
                    text = tr(
                        "It comes out for a moment, into this page only. It is not copied anywhere.",
                        "它只出来一会儿,只出现在这一页上,不会被复制到任何地方。",
                    ),
                    style = TypviaType.BodyS.style(tr.language),
                    color = Paper.ink2,
                    modifier = Modifier.padding(top = DetailMetrics.metaTop),
                )
            }
            if (onTakeOut != null && takenOut == null) {
                Text(
                    text = tr("Take it out", "取出"),
                    style = TypviaType.SectionTitle.style(tr.language),
                    color = VaultRoom.accent,
                    modifier = Modifier
                        .padding(top = DetailMetrics.verbTop)
                        .clickable(onClick = onTakeOut),
                )
            }
            return@Column
        }
        Text(
            text = body,
            style = TypviaType.Mono.style(tr.language),
            color = Paper.ink,
            modifier = Modifier.padding(top = DetailMetrics.bodyTop),
        )
        Row(
            modifier = Modifier.padding(top = DetailMetrics.verbTop),
            horizontalArrangement = Arrangement.spacedBy(DetailMetrics.verbGap),
        ) {
            if (onCopy != null) {
                Text(
                    text = tr("Copy", "复制"),
                    style = TypviaType.SectionTitle.style(tr.language),
                    color = Paper.ink,
                    modifier = Modifier.clickable { onCopy(body) },
                )
            }
            if (onEdit != null && !confirming) {
                Text(
                    text = tr("Edit", "编辑"),
                    style = TypviaType.SectionTitle.style(tr.language),
                    color = Paper.ink,
                    modifier = Modifier.clickable(onClick = onEdit),
                )
            }
            if (onPutInVault != null && !confirming) {
                Text(
                    // A verb, not a mark in the kind row: this re-encrypts the
                    // words, rebuilds the index and drops the plaintext this
                    // snippet has accumulated. Two different acts.
                    text = tr("Put it in the vault", "放进保险库"),
                    style = TypviaType.SectionTitle.style(tr.language),
                    color = VaultRoom.accent,
                    modifier = Modifier.clickable(onClick = onPutInVault),
                )
            }
            if (onTrash != null && !confirming) {
                Text(
                    // Red words and a red hairline, never a red button: the
                    // colour marks what this is, it does not invite a tap.
                    text = tr("Move to the bin", "放进回收站"),
                    style = TypviaType.BodyS.style(tr.language),
                    color = Paper.attention,
                    modifier = Modifier.clickable { confirming = true },
                )
            }
        }
        if (!confirming) aiStrip?.invoke()
        if (onTrash != null && confirming) {
            Column(modifier = Modifier.padding(top = DetailMetrics.verbTop)) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(Tokens.Line.hairlineWidth)
                        .background(Paper.attention),
                )
                Text(
                    // The real thing, by name — not "this item". A confirmation
                    // that does not say what it is about is a confirmation
                    // people learn to click through.
                    text = tr(
                        "Move \"${snippet.title}\" to the bin? It stays there for a while and can be brought back.",
                        "把「${snippet.title}」放进回收站?它会在里面待一阵子,随时能拿回来。",
                    ),
                    style = TypviaType.BodyS.style(tr.language),
                    color = Paper.ink2,
                    modifier = Modifier.padding(top = DetailMetrics.metaTop),
                )
                Row(
                    modifier = Modifier.padding(top = DetailMetrics.metaTop),
                    horizontalArrangement = Arrangement.spacedBy(DetailMetrics.verbGap),
                ) {
                    Text(
                        text = tr("Move it", "放进去"),
                        style = TypviaType.SectionTitle.style(tr.language),
                        color = Paper.attention,
                        modifier = Modifier.clickable(onClick = onTrash),
                    )
                    Text(
                        text = tr("Keep it", "留着"),
                        style = TypviaType.BodyS.style(tr.language),
                        color = Paper.ink2,
                        modifier = Modifier.clickable { confirming = false },
                    )
                }
            }
        }
    }
}

object DetailMetrics {
    val titleTop = 26.dp
    val metaTop = 8.dp
    val bodyTop = 26.dp
    val verbTop = 30.dp
    val verbGap = 26.dp
}

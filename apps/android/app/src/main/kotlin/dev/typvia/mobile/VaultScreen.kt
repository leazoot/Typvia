// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import dev.typvia.mobile.ui.BodyMask
import dev.typvia.mobile.ui.Beat
import dev.typvia.mobile.ui.Caret
import dev.typvia.mobile.ui.InkBloom
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.TypeSort
import dev.typvia.mobile.ui.TypeSortMark
import dev.typvia.mobile.ui.TypviaType
import dev.typvia.mobile.ui.pieces
import dev.typvia.mobile.ui.VaultRoom
import dev.typvia.mobile.ui.systemReduceMotion

/**
 * The vault: the one room whose material breaks from the rest of the app.
 *
 * It is an ink room in either theme. Shut, it shows three things — a caret
 * breathing slowly, one sentence, and seven dots where a count would be. No
 * lock icon, no "your data is encrypted" sales pitch, and no hint of how much
 * is in here.
 */
@Composable
fun VaultScreen(
    phase: VaultPhase,
    entries: List<VaultEntry>?,
    refusal: VaultRefusal? = null,
    shortestPassword: Int = 0,
    onMake: (String, String) -> Unit = { _, _ -> },
    onOpen: (String) -> Unit = {},
    onShut: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
    val reduceMotion = systemReduceMotion()
    // The room is *opened*, not shown: at the moment it unlocks, ink spreads
    // from the middle at the one 320ms in the product.
    //
    // Only at that moment. Walking back into a room that was already open is
    // an arrival, not an unlocking, and replaying the flourish every time
    // would turn the product's one memorable beat into furniture. Reduced
    // motion gets the room open with no circle at all.
    val bloom = remember { Animatable(Tokens.Motion.INK_BLOOM_EXTENT) }
    var previous by remember { mutableStateOf(phase) }
    LaunchedEffect(phase, reduceMotion) {
        val justUnlocked = previous != VaultPhase.Open && phase == VaultPhase.Open
        previous = phase
        if (justUnlocked && !reduceMotion) {
            bloom.snapTo(0f)
            bloom.animateTo(Tokens.Motion.INK_BLOOM_EXTENT, Beat.Unlock.enter(false))
        } else {
            bloom.snapTo(Tokens.Motion.INK_BLOOM_EXTENT)
        }
    }
    Column(
        modifier = modifier
            .fillMaxSize()
            .clip(InkBloom(bloom.value))
            .background(VaultRoom.base)
            // The way in must stay reachable with a keyboard over it: a verb
            // hidden behind the keyboard that summoned it is a door with no
            // handle.
            .imePadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = Tokens.Space.screenPadding),
    ) {
        Text(
            text = when (phase) {
                VaultPhase.Open -> tr("VAULT · OPEN", "保险库 · 已开")
                // A vault that does not exist is not a shut one. The line under
                // it says as much, and a header contradicting it is the room
                // getting its own state wrong in the reader's first glance.
                VaultPhase.Absent -> tr("VAULT · NOT MADE", "保险库 · 尚未建立")
                VaultPhase.Shut -> tr("VAULT · SHUT", "保险库 · 已锁")
            },
            style = TypviaType.MonoLabel.style(tr.language),
            color = VaultRoom.ink3,
            modifier = Modifier.padding(top = 14.dp, bottom = VaultMetrics.headGap),
        )
        when (phase) {
            VaultPhase.Shut -> Shut(refusal, onOpen)
            VaultPhase.Absent -> Absent(refusal, shortestPassword, onMake)
            VaultPhase.Open -> Opened(entries, onShut)
        }
    }
}

@Composable
private fun Shut(refusal: VaultRefusal?, onOpen: (String) -> Unit) {
    val tr = LocalTranslator.current
    var password by remember { mutableStateOf("") }
    Caret(height = 52.dp, capped = true, vaultPace = true, color = VaultRoom.accent)
    Text(
        text = tr("What is in here is only on your device.", "这里的东西只在你手上。"),
        style = TypviaType.Title2.style(tr.language),
        color = VaultRoom.ink,
        modifier = Modifier.padding(top = VaultMetrics.caretGap),
    )
    // Seven dots where a count would be. How much is in the vault is a fact
    // about the vault, and a shut vault states none.
    Text(
        text = BodyMask.SHORT,
        style = TypviaType.Mono.style(tr.language),
        color = VaultRoom.ink3,
        modifier = Modifier.padding(top = VaultMetrics.lineGap),
    )
    // The way in. It is a line to type on, not a box: the room has one thing
    // to ask and asks it the way every other field in this product does.
    SecretLine(
        label = tr("MASTER PASSWORD", "主密码"),
        value = password,
        onValueChange = { password = it },
    )
    VaultVerb(tr("Open it", "打开")) { onOpen(password) }
    VaultSaid(refusal, tr)
}

@Composable
private fun Absent(
    refusal: VaultRefusal?,
    shortestPassword: Int,
    onMake: (String, String) -> Unit,
) {
    val tr = LocalTranslator.current
    var password by remember { mutableStateOf("") }
    var again by remember { mutableStateOf("") }
    Caret(height = 52.dp, capped = true, vaultPace = true, color = VaultRoom.accent)
    Text(
        text = tr("There is no vault on this device yet.", "这台设备上还没有保险库。"),
        style = TypviaType.Title2.style(tr.language),
        color = VaultRoom.ink,
        modifier = Modifier.padding(top = VaultMetrics.caretGap),
    )
    Text(
        text = tr(
            "Save a password or a key and one is made, with a master password only you know.",
            "存下一条密码或密钥就会建起来,主密码只有你知道。",
        ),
        style = TypviaType.BodyS.style(tr.language),
        color = VaultRoom.ink2,
        modifier = Modifier.padding(top = VaultMetrics.lineGap),
    )
    SecretLine(
        // The floor comes from the vault itself, so the screen states the rule
        // that is actually enforced rather than one it remembers.
        label = if (shortestPassword > 0) {
            tr("MASTER PASSWORD · $shortestPassword+", "主密码 · $shortestPassword 起")
        } else {
            tr("MASTER PASSWORD", "主密码")
        },
        value = password,
        onValueChange = { password = it },
    )
    SecretLine(
        label = tr("AGAIN", "再来一次"),
        value = again,
        onValueChange = { again = it },
    )
    VaultVerb(tr("Make the vault", "建起保险库")) { onMake(password, again) }
    VaultSaid(refusal, tr)
}

@Composable
private fun Opened(entries: List<VaultEntry>?, onShut: () -> Unit) {
    val tr = LocalTranslator.current
    Text(
        // A count is a fact about the vault. An open vault whose list could
        // not be read has no count to state — printing 0 there would be the
        // room telling the reader their secrets are gone.
        text = when (entries) {
            null -> tr("reading…", "正在读…")
            else -> tr.pieces(entries.size)
        },
        style = TypviaType.Heading.style(tr.language),
        color = VaultRoom.ink,
    )
    for (entry in entries ?: emptyList()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = VaultMetrics.rowGap),
            horizontalArrangement = Arrangement.spacedBy(VaultMetrics.lineGap),
        ) {
            TypeSortMark(sort = TypeSort.Secret, accessibilityLabel = tr("Secret", "密钥"))
            Text(
                text = entry.title,
                style = TypviaType.SectionTitle.style(tr.language),
                color = VaultRoom.ink,
            )
        }
    }
    VaultVerb(tr("Shut it", "回锁")) { onShut() }
}

object VaultMetrics {
    val headGap = 30.dp
    val caretGap = 30.dp
    val lineGap = 18.dp
    val rowGap = 22.dp
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// The pieces the vault is asked with: a line to type a secret on, a verb, the
// sentence said when it does not open, and the door itself.
//
// They live apart from the room because the door is needed in two places —
// the room, and the page where a secret is being written. Two doors would be
// two chances to disagree about what the vault says.

package dev.typvia.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.TypviaType
import dev.typvia.mobile.ui.VaultRoom

/** One line to type a secret on: a label, the dots, and nothing else. */
@Composable
fun SecretLine(label: String, value: String, onValueChange: (String) -> Unit) {
    val tr = LocalTranslator.current
    Text(
        text = label,
        style = TypviaType.MonoLabel.style(tr.language),
        color = VaultRoom.ink3,
        modifier = Modifier.padding(top = VaultMetrics.rowGap),
    )
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        singleLine = true,
        // Typed secrets are never shown. The room's whole promise is that it
        // does not put them on screen.
        visualTransformation = PasswordVisualTransformation(),
        // And the field says what it is, so the keyboard over it behaves like
        // one is there: no suggestion strip, no autocorrect, nothing learned.
        // Masking the glyphs without declaring the field left the master
        // password sitting in a third-party keyboard's suggestion bar — this
        // product does not get to promise what it never told the platform.
        keyboardOptions = KeyboardOptions(
            keyboardType = KeyboardType.Password,
            autoCorrectEnabled = false,
        ),
        textStyle = TypviaType.Body.style(tr.language).copy(color = VaultRoom.ink),
        cursorBrush = SolidColor(VaultRoom.accent),
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = VaultMetrics.lineGap),
    )
}

/** A verb of the vault. Accent text, never a filled button. */
@Composable
fun VaultVerb(label: String, onTap: () -> Unit) {
    val tr = LocalTranslator.current
    Text(
        text = label,
        style = TypviaType.SectionTitle.style(tr.language),
        color = VaultRoom.accent,
        modifier = Modifier
            .padding(top = VaultMetrics.rowGap)
            .clickable(onClick = onTap),
    )
}

/**
 * What went wrong, said the way the delivery asks: what is still good first,
 * then a way on. No shake, no red, no count of attempts left — a refusal is
 * not a punishment, and a vault that counts down at you is one that has
 * decided you are the thief.
 */
@Composable
fun VaultSaid(refusal: VaultRefusal?, tr: Translator) {
    if (refusal == null) return
    val words = when (refusal) {
        VaultRefusal.DidNotOpen -> tr(
            "That did not open it. The vault is still shut, and nothing was lost.",
            "没能打开。库还锁着,什么都没丢。",
        )
        VaultRefusal.NotAPassword -> tr(
            "The vault would not take that as a master password.",
            "这条不能作主密码。",
        )
        VaultRefusal.TypedDifferently -> tr(
            "The two typings differ. Nothing was made.",
            "两次输入不一样。什么都没建。",
        )
        VaultRefusal.Storage -> tr(
            "Something under the room failed. The vault is unchanged.",
            "底下出了点问题。库没有变化。",
        )
    }
    Text(
        text = words,
        style = TypviaType.BodyS.style(tr.language),
        color = VaultRoom.ink2,
        modifier = Modifier.padding(top = VaultMetrics.lineGap),
    )
}

/**
 * The door, wherever it is needed.
 *
 * On the page where a secret is being written it arrives as a block of the
 * vault's own material: the material break is how this product says *this is
 * a different kind of thing*, and a door drawn on ordinary paper would be the
 * page quietly claiming the secret is ordinary too.
 *
 * @param phase what the vault is. [VaultPhase.Open] draws nothing — there is
 *   no door to show when it is already open.
 */
@Composable
fun SecretGate(
    phase: VaultPhase,
    refusal: VaultRefusal?,
    shortestPassword: Int,
    onMake: (String, String) -> Unit,
    onOpen: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (phase == VaultPhase.Open) return
    val tr = LocalTranslator.current
    var password by remember { mutableStateOf("") }
    var again by remember { mutableStateOf("") }
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(VaultRoom.base)
            .padding(horizontal = Tokens.Space.screenPadding, vertical = 8.dp),
    ) {
        Text(
            text = when (phase) {
                VaultPhase.Absent -> tr(
                    "A secret goes in the vault, and there is none on this device yet.",
                    "密钥要放进保险库,而这台设备上还没有。",
                )
                else -> tr(
                    "A secret goes in the vault, and it is shut.",
                    "密钥要放进保险库,而它锁着。",
                )
            },
            style = TypviaType.BodyS.style(tr.language),
            color = VaultRoom.ink,
            modifier = Modifier.padding(top = VaultMetrics.lineGap),
        )
        SecretLine(
            label = if (phase == VaultPhase.Absent && shortestPassword > 0) {
                tr("MASTER PASSWORD · $shortestPassword+", "主密码 · $shortestPassword 起")
            } else {
                tr("MASTER PASSWORD", "主密码")
            },
            value = password,
            onValueChange = { password = it },
        )
        if (phase == VaultPhase.Absent) {
            SecretLine(
                label = tr("AGAIN", "再来一次"),
                value = again,
                onValueChange = { again = it },
            )
            VaultVerb(tr("Make the vault and save it", "建库并存进去")) { onMake(password, again) }
        } else {
            VaultVerb(tr("Open it and save", "打开并存进去")) { onOpen(password) }
        }
        VaultSaid(refusal, tr)
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.TypviaType
import dev.typvia.mobile.ui.counted
import uniffi.typvia_mobile_ffi.VaultStatus

/**
 * What the vault chapter of settings says, as values.
 *
 * Every sentence here is about this device's own vault, and every one of them
 * is checkable — how it opens, how long it stays open, what happens when it
 * does not. Nothing is a reassurance: "your data is encrypted" is a sentence
 * that appears on products whose data is not.
 */
object VaultChapterCopy {
    /**
     * The idle window, said in whole minutes.
     *
     * Rounded down and never to zero: a window of forty seconds said as "0
     * minutes" reads as "it does not re-lock", which is the opposite of true.
     */
    fun idleWindow(status: VaultStatus?, tr: Translator): String? {
        val ms = status?.idleTimeoutMs ?: return null
        if (ms <= 0) return null
        val minutes = ms / 60_000
        return if (minutes < 1) {
            tr("under a minute", "不到一分钟")
        } else {
            tr.counted(minutes.toULong(), "minute", "minutes", "$minutes 分钟")
        }
    }

    /** What the chapter says at the top, in one sentence per phase. */
    fun headline(phase: VaultPhase, tr: Translator): String = when (phase) {
        VaultPhase.Absent -> tr(
            "There is no vault on this device yet.",
            "这台设备上还没有保险库。",
        )
        VaultPhase.Shut -> tr(
            "It is shut. Nothing comes out until you open it.",
            "它锁着。不打开就什么都不出来。",
        )
        VaultPhase.Open -> tr(
            "It is open, and it shuts itself when you stop using it.",
            "它开着,而你不再用它时它会自己关上。",
        )
    }
}

/**
 * The vault chapter: a whole page, as the delivery has it, rather than a
 * folding panel on the contents page.
 *
 * It is drawn on ordinary paper, not in the ink room: this is a page *about*
 * the vault, and the material break belongs to the vault itself. A settings
 * page wearing the vault's material would be borrowing weight it has not
 * earned.
 */
@Composable
fun VaultChapterScreen(
    phase: VaultPhase,
    status: VaultStatus?,
    onClose: () -> Unit,
    onShut: (() -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
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
                .padding(top = 14.dp, bottom = SettingsMetrics.headGap)
                .clickable(onClick = onClose),
        )
        // The numeral stays as the page's own heading — the delivery keeps it
        // in the corner rather than replacing it with a title.
        Text(
            text = "04",
            style = TypviaType.Title1.style(tr.language),
            color = Paper.ink3,
        )
        Text(
            text = tr("Vault", "保险库"),
            style = TypviaType.Title2.style(tr.language),
            color = Paper.ink,
            modifier = Modifier.padding(top = SettingsMetrics.rowGap),
        )
        Text(
            text = VaultChapterCopy.headline(phase, tr),
            style = TypviaType.BodyS.style(tr.language),
            color = Paper.ink2,
            modifier = Modifier.padding(top = SettingsMetrics.rowGap),
        )

        Line(
            label = tr("HOW IT OPENS", "怎么打开"),
            // Said plainly, and this is the whole of it on this platform: a
            // page listing a fingerprint option this device does not have
            // would be describing a different product.
            value = tr("Master password", "主密码"),
        )
        // Stated only once there is a vault to shut. A device with none has no
        // window to announce — the number would be a setting for something
        // that does not exist yet.
        VaultChapterCopy.idleWindow(status.takeIf { phase != VaultPhase.Absent }, tr)?.let { window ->
            Line(
                label = tr("IT SHUTS AFTER", "多久回锁"),
                value = window,
            )
        }
        if (phase == VaultPhase.Open && onShut != null) {
            Text(
                text = tr("Shut it now", "现在就锁上"),
                style = TypviaType.SectionTitle.style(tr.language),
                color = dev.typvia.mobile.ui.VaultRoom.accent,
                modifier = Modifier
                    .padding(top = Tokens.Space.group)
                    .clickable(onClick = onShut),
            )
        }
        Text(
            // The one thing worth stating outright, because it is the promise
            // the whole room exists to keep — and it is checkable, which is
            // why it is worth printing at all.
            text = tr(
                "What is in here is encrypted on this device. It is never sent to a model, and it is never put on the clipboard.",
                "这里的东西在这台设备上加密。它不会被送去任何模型,也不会进剪贴板。",
            ),
            style = TypviaType.Caption.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = Tokens.Space.group, bottom = Tokens.Space.section),
        )
    }
}

/** One stated fact: a caps mono label with the value under it. */
@Composable
private fun Line(label: String, value: String) {
    val tr = LocalTranslator.current
    Text(
        text = label,
        style = TypviaType.MonoLabel.style(tr.language),
        color = Paper.ink3,
        modifier = Modifier.padding(top = Tokens.Space.group),
    )
    Text(
        text = value,
        style = TypviaType.Body.style(tr.language),
        color = Paper.ink,
        modifier = Modifier.padding(top = SettingsMetrics.rowGap),
    )
}

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
import androidx.compose.foundation.layout.fillMaxWidth
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
import dev.typvia.mobile.ui.TypviaType
import uniffi.typvia_mobile_ffi.SyncDevice
import uniffi.typvia_mobile_ffi.SyncStatus

/**
 * The sync chapter: a whole page.
 *
 * The delivery's own words for this page are the model — one sentence that
 * finishes the encryption story without a shield or the phrase "military
 * grade", then the devices this key knows, then the ways on. Switches appear
 * only at this level, and at most two of them.
 */
@Composable
fun SyncChapterScreen(
    status: SyncStatus?,
    devices: List<SyncDevice>?,
    lastRound: String?,
    isWorking: Boolean,
    onClose: () -> Unit,
    onSyncNow: () -> Unit,
    onSwitch: (Boolean) -> Unit,
    onJoin: () -> Unit,
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
        Text(
            text = "01",
            style = TypviaType.Title1.style(tr.language),
            color = Paper.ink3,
        )
        Text(
            text = tr("Sync", "同步"),
            style = TypviaType.Title2.style(tr.language),
            color = Paper.ink,
            modifier = Modifier.padding(top = SettingsMetrics.rowGap),
        )
        Text(
            // The whole encryption story in one sentence, as the delivery
            // writes it: no shield, no "military grade", and the part people
            // do not expect — the titles are inside the ciphertext too.
            text = tr(
                "A snippet is encrypted before it leaves this device. The server keeps bytes it cannot read — the titles are in there too.",
                "片段在离开这台设备之前就已经加密。服务器只保存看不懂的字节,连标题也在里面。",
            ),
            style = TypviaType.BodyS.style(tr.language),
            color = Paper.ink2,
            modifier = Modifier.padding(top = SettingsMetrics.rowGap),
        )
        Text(
            text = SyncChapterCopy.headline(status, tr),
            style = TypviaType.Body.style(tr.language),
            color = Paper.ink,
            modifier = Modifier.padding(top = Tokens.Space.group),
        )
        lastRound?.let { said ->
            Text(
                text = said,
                style = TypviaType.Mono.style(tr.language),
                color = Paper.ink3,
                modifier = Modifier.padding(top = SettingsMetrics.rowGap),
            )
        }

        // The way in for a device that has no account yet. It is offered only
        // where it can be taken: a phone that cannot keep a sync key has
        // nothing to join with, and a phone that already joined has nothing to
        // join.
        if (SyncChapterCopy.offersJoining(status)) {
            PaperVerb(
                isPrimary = true,
                label = tr("Join an account", "加入一个账户"),
                enabled = !isWorking,
                onTap = onJoin,
            )
        }

        // Only the ways on that exist. A page that offers to sync a device
        // which cannot hold a key teaches the reader not to trust the next
        // button either.
        if (status?.available == true && status.configured) {
            PaperVerb(
                label = if (isWorking) {
                    tr("Syncing…", "正在同步…")
                } else {
                    tr("Sync now", "现在同步")
                },
                enabled = !isWorking && status.enabled,
                onTap = onSyncNow,
            )
            PaperVerb(
                label = if (status.enabled) {
                    tr("Switch sync off", "关掉同步")
                } else {
                    tr("Switch sync on", "打开同步")
                },
                enabled = !isWorking,
                onTap = { onSwitch(!status.enabled) },
            )
        }

        if (!devices.isNullOrEmpty()) {
            Text(
                text = tr("THE DEVICES THIS KEY KNOWS", "这把钥匙认得的设备"),
                style = TypviaType.MonoLabel.style(tr.language),
                color = Paper.ink3,
                modifier = Modifier.padding(top = Tokens.Space.group),
            )
            for (device in devices) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = SettingsMetrics.rowGap),
                ) {
                    Text(
                        text = device.name,
                        style = TypviaType.SectionTitle.style(tr.language),
                        color = Paper.ink,
                    )
                    Text(
                        text = SyncChapterCopy.deviceMeta(device, tr),
                        style = TypviaType.Mono.style(tr.language),
                        color = Paper.ink3,
                    )
                }
            }
        }
        Text(
            // Said last, because it is the sentence that makes the switch above
            // safe to touch.
            text = tr(
                "Sync only affects the copies on your other devices. Switch it off and this one works exactly as it did.",
                "同步只影响其它设备上的副本。关掉它,这一台照常工作。",
            ),
            style = TypviaType.Caption.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = Tokens.Space.group, bottom = Tokens.Space.section),
        )
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ffi.TypviaStore
import dev.typvia.mobile.ui.Translator
import uniffi.typvia_mobile_ffi.SyncDevice
import uniffi.typvia_mobile_ffi.SyncRound
import uniffi.typvia_mobile_ffi.SyncStatus

/**
 * What the sync chapter says, as values.
 *
 * The rule this file exists to hold: **the page reports, it never reassures**.
 * Every line is a fact the reader could check — how many are waiting, when the
 * last round was, which devices the key knows — and where a fact is missing it
 * is missing rather than filled in with an optimistic default.
 */
object SyncChapterCopy {
    /**
     * The one-line state, in the order the delivery states it: what is still
     * good first, then what happened.
     */
    fun headline(status: SyncStatus?, tr: Translator): String = when {
        status == null -> tr(
            "Everything is on this device. Whether it can be shared could not be read just now.",
            "东西都在这台设备上。能不能分享出去,刚才没读出来。",
        )
        // A device with no gated key store cannot hold a sync key at all.
        // Saying "not set up" there would invite the reader to go and set up
        // something this phone cannot do.
        !status.available -> tr(
            "This device cannot keep a sync key, so everything here stays here.",
            "这台设备存不住同步密钥,所以东西只留在这里。",
        )
        !status.configured -> tr(
            "Not set up. Everything works without it.",
            "还没设置。不设置也照常能用。",
        )
        !status.enabled -> tr(
            "Switched off. This device works exactly as it did.",
            "已关闭。这台设备照常工作。",
        )
        // One of them is a sentence of its own: the verb agrees as well as the
        // noun, which is why this is not the shared noun helper.
        status.pendingBacklog == 1uL -> tr(
            "1 change is queued. It goes out when the network comes back.",
            "有 1 条改动排在队里,网络回来会自己送出去。",
        )
        status.pendingBacklog > 0uL -> tr(
            "${status.pendingBacklog} changes are queued. They go out when the network comes back.",
            "有 ${status.pendingBacklog} 条改动排在队里,网络回来会自己送出去。",
        )
        else -> tr("Up to date.", "已是最新。")
    }

    /**
     * Whether this page may offer to join an account.
     *
     * Three noes, each for its own reason: a status that could not be read
     * makes no claim about what is possible, a device with no gated key store
     * has nothing to hold a sync key in, and a device that already has an
     * account is not looking for one. Offering it anywhere else is offering a
     * way on that ends in a refusal the reader cannot act on.
     */
    fun offersJoining(status: SyncStatus?): Boolean =
        status != null && status.available && !status.configured

    /** What one round did, in the fewest words that are true. */
    fun round(round: SyncRound?, tr: Translator): String? {
        if (round == null) return null
        return tr(
            "Sent ${round.pushed}, received ${round.applied}.",
            "送出 ${round.pushed} 条,收到 ${round.applied} 条。",
        )
    }

    /** One device row's second line: which one it is, and whether it is trusted. */
    fun deviceMeta(device: SyncDevice, tr: Translator): String {
        val words = buildList {
            if (device.isThisDevice) add(tr("this one", "这一台"))
            add(device.platform)
            // Said only when it is *not* true: a verified device is the
            // ordinary case, and printing "verified" on every row turns the
            // word into decoration.
            if (!device.verified) add(tr("unverified", "未验证"))
            if (device.revokedAt != null) add(tr("removed", "已移除"))
        }
        return words.joinToString(" · ")
    }
}

/**
 * The acts behind the sync page.
 *
 * No rule about syncing lives here — what a round does, what may be sent, what
 * a conflict is are all the core's. This carries the question across and hands
 * back what came, including the failures: **a sync page that swallows an error
 * is a page that says "up to date" about a library that is not.**
 */
object SyncKeeper {
    suspend fun status(store: TypviaStore): SyncStatus? =
        runCatching { store.perform { it.syncStatus() } }.getOrNull()

    suspend fun devices(store: TypviaStore): List<SyncDevice>? =
        runCatching { store.perform { it.syncDevices() } }.getOrNull()

    suspend fun runRound(store: TypviaStore): Result<SyncRound> =
        runCatching { store.perform { it.syncNow() } }

    suspend fun switchOff(store: TypviaStore): Result<SyncStatus> =
        runCatching { store.perform { it.syncDisable() } }

    suspend fun switchOn(store: TypviaStore): Result<SyncStatus> =
        runCatching { store.perform { it.syncResume() } }
}

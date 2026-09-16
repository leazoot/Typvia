// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ffi.TypviaStore
import dev.typvia.mobile.ui.Translator
import uniffi.typvia_mobile_ffi.CoreException
import uniffi.typvia_mobile_ffi.PairingClaim
import uniffi.typvia_mobile_ffi.PairingStart
import uniffi.typvia_mobile_ffi.SyncStatus
import uniffi.typvia_mobile_ffi.webdavCredentials
import kotlin.math.ceil

/** Where the ciphertext this device is joining actually lives. */
enum class PairingTarget {
    /** A Typvia sync server: its address and the account id, both readable on
     * the device that already holds the library. */
    Server,

    /** A WebDAV folder: the same folder the other device uses. The account is
     * read from the storage rather than typed. */
    Webdav,
}

/**
 * Where a pairing is up to.
 *
 * The two numbered steps are showing a code and comparing the check string.
 * Nothing is installed before the second one is answered — which is why it is
 * the step the screen has no way past.
 */
sealed interface PairingStep {
    data class Offering(val code: String) : PairingStep

    data class Comparing(val sas: String, val fingerprint: String) : PairingStep

    data object Joined : PairingStep

    /** The reader said the two sides differ, or the round failed. The snippets
     * on both devices are untouched either way. */
    data object Dropped : PairingStep

    /** Which of the two numbered steps this is. A drop stays on the first: the
     * reader is back where a new code is asked for, not further along. */
    val number: Int
        get() = when (this) {
            is Offering, Dropped -> 1
            is Comparing, Joined -> 2
        }

    companion object {
        const val TOTAL = 2
    }
}

/**
 * The code on screen, and how long it is good for.
 *
 * @param secondsLeft null when **nobody promised a window** — a WebDAV folder
 *   has no session server, and a screen counting one down anyway would be
 *   stating an expiry no one undertook.
 */
data class PairingCode(val code: String, val secondsLeft: Long?) {
    /** Codes stop being valid; they do not quietly refresh themselves behind
     * the reader's back, because a code that changed while it was being read
     * aloud is a code that will not work. */
    val hasExpired: Boolean get() = secondsLeft != null && secondsLeft <= 0

    /** Rounded up rather than divided down: a code with forty seconds left is
     * good for a minute, and telling a reader "0 minutes" while it still works
     * is telling them it is dead. */
    val minutesLeft: Int
        get() = secondsLeft?.let { ceil(it / 60.0).toInt().coerceAtLeast(0) } ?: 0

    companion object {
        /** Grouped for reading aloud. A wall of characters is read wrong once
         * and then blamed on the product. */
        fun grouped(code: String, every: Int = 4): String =
            if (every <= 0) code else code.chunked(every).joinToString(" ")
    }
}

/**
 * One group of the check string, and how much of it has landed.
 *
 * A group is never split across lines: the reader is comparing it against
 * another screen, and half a group on each line is how a comparison is read
 * wrong.
 */
class SasGroup(val characters: List<Char>, shown: Int) {
    val shown: Int = shown.coerceIn(0, characters.size)

    /**
     * The group as it is set, characters that have not landed yet holding
     * their place. The face is mono, so a blank is exactly as wide as a
     * letter and nothing shifts sideways as the rest arrive.
     */
    val text: String
        get() = characters.mapIndexed { index, char -> if (index < shown) char else ' ' }
            .joinToString(separator = "")
}

/**
 * The check string, arriving one character at a time.
 *
 * Each half is worked out by one of the two devices, so the pair agreeing is
 * the proof that nothing sat in the middle. They arrive in sequence because
 * arriving at once invites a glance, and the reader is being asked to compare.
 *
 * How long the string is belongs to the core, not to this screen: it hands
 * over groups joined by hyphens and this lays out however many it gets. The
 * hyphens are separators rather than something to compare, so they are dropped
 * here and the gap between groups says the same thing. Laying the groups out
 * in rows is the whole point — a string set on one line runs off the edge of
 * the phone, and characters the reader cannot see are characters they cannot
 * compare.
 */
class SasReveal(sas: String, shown: Int) {
    /** What the reader compares: the characters, hyphens dropped. */
    val characters: List<Char> = sas.filter { it != GROUP_SEPARATOR }.toList()
    val shown: Int = shown.coerceIn(0, characters.size)

    /** The groups, packed into rows that fit. */
    val rows: List<List<SasGroup>> = packIntoRows(groupsOf(sas, this.shown))

    /** How far the line under the characters has grown. */
    val progress: Float
        get() = if (characters.isEmpty()) 0f else shown.toFloat() / characters.size

    val isComplete: Boolean get() = shown == characters.size && characters.isNotEmpty()

    /**
     * What a reader who cannot see the screen is told, groups separated so the
     * reading has somewhere to breathe. Only what has landed is spoken: this
     * screen never claims a character it has not shown.
     */
    val spoken: String
        get() = rows.flatten()
            .map { group -> group.characters.take(group.shown).joinToString(separator = "") }
            .filter { it.isNotEmpty() }
            .joinToString(separator = " ")

    companion object {
        /** The window the delivery spreads the reveal across. */
        const val WINDOW_MILLIS = 720L

        /** How the core joins the groups it hands over. */
        private const val GROUP_SEPARATOR = '-'

        /**
         * How many characters a row carries. Ten of them, set at the code rung,
         * fit the narrowest phone this app runs on at the system's standard
         * text sizes — the check has to be makeable there too, not only on a
         * roomy screen. Past those sizes the row is the same shape every other
         * screen here is: it needs the phone's own text scaling honoured, which
         * is a debt of this app rather than of this page.
         */
        const val CHARACTERS_PER_ROW = 10

        fun stepMillis(count: Int): Long =
            if (count <= 0) WINDOW_MILLIS else WINDOW_MILLIS / count

        /** Splits on the separator and tells each group how much of it landed. */
        private fun groupsOf(sas: String, shown: Int): List<SasGroup> {
            var landed = 0
            return sas.split(GROUP_SEPARATOR).filter { it.isNotEmpty() }.map { text ->
                val group = SasGroup(text.toList(), shown - landed)
                landed += text.length
                group
            }
        }

        private fun packIntoRows(groups: List<SasGroup>): List<List<SasGroup>> {
            val rows = mutableListOf<MutableList<SasGroup>>()
            var width = 0
            for (group in groups) {
                val row = rows.lastOrNull()
                if (row == null || width + group.characters.size > CHARACTERS_PER_ROW) {
                    rows.add(mutableListOf(group))
                    width = group.characters.size
                } else {
                    row.add(group)
                    width += group.characters.size
                }
            }
            return rows
        }
    }
}

/**
 * What this device has been asked to join, as the reader typed it.
 *
 * No rule about addresses lives here — what a usable address is, what an
 * account id has to look like, whether a folder holds an account are all the
 * core's answers. This only knows that empty fields have nothing to send.
 */
data class PairingAsk(
    val target: PairingTarget = PairingTarget.Server,
    val address: String = "",
    val accountId: String = "",
    val username: String = "",
    val password: String = "",
) {
    /**
     * Whether there is enough to ask with.
     *
     * @param canHoldAKey null while it is unknown, **including when the read
     *   failed**. Only a definite no stops the reader: unknown lets them try
     *   and the attempt answers honestly, which is better than a screen
     *   refusing on the strength of a question it never got an answer to.
     */
    fun canBegin(canHoldAKey: Boolean?): Boolean {
        if (canHoldAKey == false) return false
        if (address.isBlank()) return false
        return target != PairingTarget.Server || accountId.isNotBlank()
    }
}

/**
 * Why the join did not happen, in a shape a screen can render.
 *
 * Never the raw error: an engine message is written for whoever reads a log,
 * and it can name a server address or an account.
 */
enum class PairingRefusal {
    /** The address answered nothing. Offline is a state, not a fault. */
    NoAnswer,

    /** Reached, and it would not have this device — wrong account, dead code,
     * a session that has already been answered. */
    NotThisAccount,

    /** The vault material arrived and this device's master password did not
     * unwrap it. */
    NotTheMasterPassword,

    /** The core would not take what was typed as an address or an id. */
    NotAnAddress,

    /** Something under the page failed. Nothing was installed. */
    Storage,
    ;

    companion object {
        fun of(error: Throwable): PairingRefusal = when (error) {
            is CoreException.Unavailable -> NoAnswer
            is CoreException.Conflict, is CoreException.NotFound -> NotThisAccount
            is CoreException.PermissionDenied -> NotTheMasterPassword
            is CoreException.Validation -> NotAnAddress
            else -> Storage
        }
    }
}

/**
 * What the pairing page says, as values.
 *
 * The rule this holds: **every refusal ends by saying nothing was installed**.
 * A reader who has just been refused mid-way through joining an account does
 * not know whether their library is now half somebody else's, and that is the
 * question the sentence has to answer before it says anything else.
 */
object PairingCopy {
    /** The sentence for a refusal, and the sentence that follows every one of
     * them. */
    fun sentence(refusal: PairingRefusal, tr: Translator): String {
        val said = when (refusal) {
            PairingRefusal.NoAnswer -> tr(
                "Nothing answered at that address.",
                "那个地址没有应答。",
            )
            PairingRefusal.NotThisAccount -> tr(
                "That account would not take this device. The code may have been used already.",
                "那个账户没有接受这台设备。那个码可能已经被用过了。",
            )
            PairingRefusal.NotTheMasterPassword -> tr(
                "That master password did not open what arrived.",
                "这个主密码打不开送过来的东西。",
            )
            PairingRefusal.NotAnAddress -> tr(
                "That is not an address this can join.",
                "这条不是能加入的地址。",
            )
            PairingRefusal.Storage -> tr(
                "Something under the page failed.",
                "底下出了点问题。",
            )
        }
        return "$said ${nothingInstalled(tr)}"
    }

    /** What is true after every refusal, and after every drop. */
    fun nothingInstalled(tr: Translator): String =
        tr("Nothing was installed here.", "这台设备上什么都没装。")

    /**
     * How long the code is good for, or nothing at all.
     *
     * A code nobody promised a window for says nothing about validity — the
     * alternative is a screen announcing "0 minutes" over a code that works.
     */
    fun validity(code: PairingCode, tr: Translator): String? = when {
        code.secondsLeft == null -> null
        code.hasExpired -> tr(
            "This code has expired — ask for a new one",
            "这个码过期了 · 重新出码",
        )
        code.minutesLeft == 1 -> tr("Good for 1 more minute", "1 分钟内有效")
        else -> tr(
            "Good for ${code.minutesLeft} more minutes",
            "${code.minutesLeft} 分钟内有效",
        )
    }
}

/**
 * The acts behind joining an account.
 *
 * No rule about pairing lives here: the key exchange, the check string, what
 * a valid code is and what finalizing installs are the core's. This carries
 * the question across and hands back what came, failures included.
 */
object PairingKeeper {
    /**
     * How often the joining device asks whether the other one has answered.
     * The same two seconds the desktop uses — both are waiting on one server.
     */
    const val POLL_SECONDS = 2L

    /**
     * Whether this device can keep a sync key at all.
     *
     * Null when the read failed, and that is not the same as false: "this
     * device cannot keep a key" is a claim about the device, not a way of
     * saying the question went unanswered.
     */
    suspend fun canHoldAKey(store: TypviaStore): Boolean? =
        runCatching { store.perform { it.syncStatus() }.available }.getOrNull()

    /** Starts the session and returns the code to hold up. */
    suspend fun begin(store: TypviaStore, ask: PairingAsk): Result<PairingStart> = runCatching {
        val address = ask.address.trim()
        store.perform { core ->
            when (ask.target) {
                PairingTarget.Server -> core.pairingBegin(address, ask.accountId.trim())
                // The credentials string is packed by the sync crate, never
                // spelled out here: its shape belongs to the transport, and a
                // screen writing its own copy will still be writing the old
                // one after it changes.
                PairingTarget.Webdav -> core.pairingBeginWebdav(
                    address,
                    webdavCredentials(ask.username.trim(), ask.password),
                )
            }
        }
    }

    /**
     * Asks whether the other device has answered.
     *
     * A failed poll is the other device not having answered yet, or the
     * network being away. Neither is news: the page keeps waiting and keeps
     * saying that nothing has been installed.
     */
    suspend fun poll(store: TypviaStore): PairingClaim? =
        runCatching { store.perform { it.pairingPoll() } }.getOrNull()

    /**
     * Installs, after the reader said the check string matches. The only path
     * that changes anything on this device.
     *
     * @param masterPassword this device's own, for the case where the offer
     *   carries vault material: the master key is re-wrapped under it and
     *   never travels in the clear. Blank means the account has no vault.
     */
    suspend fun finalize(store: TypviaStore, masterPassword: String): Result<SyncStatus> =
        runCatching {
            store.perform { it.pairingFinalize(masterPassword.ifBlank { null }) }
        }

    /** Abandons the session on this device. Nothing was installed. */
    suspend fun cancel(store: TypviaStore) {
        runCatching { store.perform { it.pairingCancel() } }
    }
}

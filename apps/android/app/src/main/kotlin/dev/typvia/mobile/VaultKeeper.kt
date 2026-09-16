// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ffi.TypviaStore
import uniffi.typvia_mobile_ffi.CoreException
import uniffi.typvia_mobile_ffi.VaultStatus
import uniffi.typvia_mobile_ffi.masterPasswordMinLength

/**
 * Opening and making the vault.
 *
 * The room could already draw all three of its faces, and there was no way to
 * reach two of them: this device could not make a vault and could not open
 * one, so nothing could ever be put in it or taken out. This is the way in.
 *
 * No rule about passwords lives here. How long a master password has to be,
 * what counts as the right one, what a failed attempt costs — every one of
 * those is the core's answer, asked for rather than guessed at, so the two
 * platforms cannot come to disagree about the door to the same vault.
 */
object VaultKeeper {
    /**
     * How short a master password may not be. Read from the core so the screen
     * states the rule that is actually enforced rather than one it remembers.
     */
    fun shortestPassword(): Int = masterPasswordMinLength().toInt()

    suspend fun status(store: TypviaStore): VaultStatus? =
        runCatching { store.perform { it.vaultStatus() } }.getOrNull()

    /**
     * Makes the vault this device does not have yet.
     *
     * @return the refusal, when there is one. The password is **not** checked
     *   here first: a screen that pre-judges the length prints its own opinion,
     *   and the day the core's floor moves the two disagree.
     */
    suspend fun make(store: TypviaStore, password: String): Result<VaultStatus> =
        runCatching { store.perform { it.vaultInitialize(password) } }

    suspend fun open(store: TypviaStore, password: String): Result<VaultStatus> =
        runCatching { store.perform { it.vaultUnlockPassword(password) } }

    suspend fun shut(store: TypviaStore): Result<VaultStatus> =
        runCatching { store.perform { it.vaultLock() } }

    /** The names and the last time each was taken out. Never a body. */
    suspend fun list(store: TypviaStore): List<VaultEntry>? =
        runCatching {
            store.perform { it.vaultList(VAULT_PAGE, 0u) }.map {
                VaultEntry(id = it.id, title = it.title, lastUsedAt = it.lastUsedAt)
            }
        }.getOrNull()

    /**
     * Takes one secret's words out.
     *
     * They are handed to the caller and nowhere else: not to the clipboard,
     * not to a log, not into any list this app keeps. Whoever asks is
     * responsible for putting them down again.
     */
    suspend fun takeOut(store: TypviaStore, id: String): Result<String> =
        runCatching { store.perform { it.vaultReveal(id) } }

    /**
     * Moves a snippet that is already saved into the vault.
     *
     * The core's own act — re-encrypt, rebuild the index, drop the plaintext
     * versions this snippet has accumulated — asked for, never imitated here.
     * Imitating it would leave the old plaintext searchable, which is the
     * whole thing the reader was trying to undo.
     */
    suspend fun putIn(store: TypviaStore, id: String): Result<Unit> =
        runCatching { store.perform { it.snippetConvertToSensitive(id) } }

    /**
     * How long words stay out before the page covers them again.
     *
     * Long enough to read and type somewhere else; short enough that a phone
     * left on a table does not keep showing a key. The number is the
     * delivery's.
     */
    const val TAKEN_OUT_SECONDS = 20L

    /**
     * Two typings of the same new password.
     *
     * This one *is* the screen's: it is not a rule about passwords, it is the
     * one guard against a reader locking their own secrets behind a typo they
     * cannot see. Nothing is sent anywhere when the two differ.
     */
    fun typedTwiceMatches(password: String, confirmation: String): Boolean =
        password == confirmation

    /** How much of the vault one page asks for. */
    private const val VAULT_PAGE: UInt = 200u
}

/**
 * Why the door did not open, in a shape a screen can render.
 *
 * The reason is never the raw error: a refusal that shows a message from the
 * inside of the core is a refusal in the wrong language, and often one that
 * says more about the vault than the vault should say.
 */
enum class VaultRefusal {
    /** The password did not open it. Not "wrong password" — the vault does not
     * distinguish, and neither does this. */
    DidNotOpen,

    /** The core would not take that as a master password (too short). */
    NotAPassword,

    /** The two typings differ. Nothing was sent. */
    TypedDifferently,

    /** Something under the room failed. The vault is unchanged. */
    Storage,
    ;

    companion object {
        fun of(error: Throwable): VaultRefusal = when (error) {
            is CoreException.PermissionDenied -> DidNotOpen
            is CoreException.Validation -> NotAPassword
            else -> Storage
        }
    }
}

/**
 * A secret typed out before the vault could take it.
 *
 * Held rather than dropped: a reader who is asked for a master password in the
 * middle of writing something down has not changed their mind about the words.
 */
data class PendingSecret(
    val title: String,
    val body: String,
    val trigger: String?,
    val folderId: String?,
)

/**
 * Finishes the write the vault was standing in the way of.
 *
 * @param settled true when the secret is in. False is a real failure with the
 *   vault already open — the words stay on screen and the page says so.
 */
suspend fun finishSecret(
    store: dev.typvia.mobile.ffi.TypviaStore,
    held: PendingSecret,
    settled: suspend (Boolean) -> Unit,
) {
    val written = SnippetWriter.writeSecret(
        store,
        title = held.title,
        body = held.body,
        trigger = held.trigger?.takeIf { it.isNotBlank() },
        folderId = held.folderId,
    )
    settled(written.isSuccess)
}

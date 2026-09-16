// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ffi.TypviaStore
import dev.typvia.mobile.ui.TypeSort
import uniffi.typvia_mobile_ffi.Snippet
import uniffi.typvia_mobile_ffi.SnippetDraft
import uniffi.typvia_mobile_ffi.SnippetEdit

/**
 * Writing one snippet down.
 *
 * It exists apart from the screen so the act can be tested without one — and
 * so the screen stays what it should be: fields and a verb. No rule lives
 * here either: the kind a body is filed under, the name an untitled snippet
 * takes, what makes a trigger valid are all the core's answers.
 */
object SnippetWriter {
    /**
     * Makes a folder.
     *
     * The name goes over as typed: whether it is a name at all — trimmed, and
     * whether anything is left — is the core's answer, the same one every host
     * gets. This screen only asks.
     */
    suspend fun makeFolder(store: TypviaStore, name: String): Result<uniffi.typvia_mobile_ffi.Folder> =
        runCatching {
            store.perform { core -> core.folderCreate(name, null, 0) }
        }

    /**
     * Says that a snippet was actually delivered.
     *
     * The rule about when a use counts is the shared layer's; this only tells
     * it. A failure is not worth telling the reader about — the words are
     * already where they wanted them, and a toast about bookkeeping would be
     * the product talking about itself.
     */
    suspend fun recordUse(store: TypviaStore, id: String) {
        runCatching { store.perform { it.snippetRecordUse(id) } }
    }

    /**
     * Writes a secret.
     *
     * A different door on purpose: the ordinary one refuses the secret kind
     * outright, in the core, so this guarantee does not rest on a screen
     * remembering to route correctly. What comes back carries no body — the
     * words are encrypted on the way in and the row hands back nothing.
     *
     * @return the refusal when the vault is shut or absent. Deciding what to
     *   do about that is the screen's; making the vault is not this call's
     *   business to do quietly on the reader's behalf.
     */
    suspend fun writeSecret(
        store: TypviaStore,
        title: String,
        body: String,
        trigger: String? = null,
        folderId: String? = null,
    ): Result<Snippet> = runCatching {
        store.perform { core ->
            core.vaultCreateSecret(
                SnippetDraft(
                    title = title,
                    body = body,
                    snippetType = TypeSort.Secret.coreType,
                    description = null,
                    folderId = folderId,
                    trigger = trigger,
                    triggerMode = null,
                    language = null,
                ),
            )
        }
    }

    /**
     * Saves an edit to one that already exists.
     *
     * Everything the reader did not touch travels back unchanged — the kind,
     * the trigger, the folder, the flags. An edit screen that sends its own
     * defaults for the fields it does not show is an edit screen that quietly
     * resets them.
     */
    suspend fun update(
        store: TypviaStore,
        snippet: Snippet,
        title: String,
        body: String,
        sort: TypeSort? = null,
        trigger: String? = null,
        folderId: String? = null,
    ): Result<Snippet> = runCatching {
        store.perform { core ->
            core.snippetUpdate(
                SnippetEdit(
                    id = snippet.id,
                    title = title,
                    body = body,
                    snippetType = sort?.coreType ?: snippet.snippetType,
                    description = snippet.description,
                    folderId = folderId ?: snippet.folderId,
                    trigger = trigger ?: snippet.trigger,
                    triggerMode = snippet.triggerMode,
                    language = snippet.language,
                    isFavorite = snippet.isFavorite,
                    isPinned = snippet.isPinned,
                    isEnabled = snippet.isEnabled,
                ),
            )
        }
    }

    /**
     * Empties the bin, one row at a time.
     *
     * Deliberately not a single "purge everything" call: each row is deleted
     * on its own, so a failure part-way leaves the rest of the bin intact and
     * the screen can say how far it got instead of claiming a clean sweep it
     * did not make.
     *
     * @return how many were actually destroyed.
     */
    suspend fun emptyTheBin(store: TypviaStore, ids: List<String>): Int =
        store.perform { core ->
            ids.count { id -> runCatching { core.snippetDeleteForever(id) }.isSuccess }
        }

    /**
     * Moves one snippet to the recycle bin.
     *
     * Not a delete: the core keeps it for a retention window, which is what
     * makes an inline confirmation enough. A screen that said "delete" here
     * would be describing something harsher than what happens.
     */
    suspend fun trash(store: TypviaStore, id: String): Result<Unit> =
        runCatching { store.perform { core -> core.snippetTrash(id) } }

    /**
     * @param title as typed. Empty is allowed: the shared layer files an
     *   untitled snippet under its first line — one rule, both platforms.
     */
    suspend fun write(
        store: TypviaStore,
        title: String,
        body: String,
        sort: TypeSort = TypeSort.Text,
        trigger: String = "",
        folderId: String? = null,
    ): Result<Snippet> = runCatching {
        store.perform { core ->
            core.snippetCreate(
                SnippetDraft(
                    title = title,
                    body = body,
                    snippetType = sort.coreType,
                    description = null,
                    folderId = folderId,
                    // Sent as typed and as given: whether an empty field is a
                    // trigger, and what kind of trigger it is when nobody
                    // said, are both the shared layer's answers.
                    trigger = trigger,
                    triggerMode = null,
                    language = null,
                ),
            )
        }
    }
}

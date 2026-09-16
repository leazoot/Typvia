// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ffi

import java.io.File
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.withContext
import java.util.concurrent.Executors
import uniffi.typvia_mobile_ffi.TypviaCore

/**
 * The app's one handle on its data.
 *
 * Everything this type does is about *when* and *where* a call runs, never
 * about what it means. The rules — what a valid trigger is, when a version is
 * recorded, what gets indexed, in which order a secret is encrypted — all live
 * behind the bridge, and restating any of them here would give the product two
 * answers to the same question.
 *
 * The single-thread dispatcher is the storage rule made mechanical: SQLite has
 * one writer, so calls queue rather than race, and they run off the main thread
 * so a query never costs a frame. It is the same shape as the iOS side's actor
 * — one process, one writer, one place that says so.
 */
class TypviaStore private constructor(
    private val core: TypviaCore,
    private val dispatcher: CoroutineDispatcher,
) {
    /**
     * Runs one call against the core.
     *
     * The bridge is already a typed surface, so this hands it over as-is
     * instead of restating thirty signatures that would drift from it.
     */
    suspend fun <T> perform(body: (TypviaCore) -> T): T = withContext(dispatcher) { body(core) }

    companion object {
        /**
         * Opens the database under [dataDirectory], creating it on first run
         * and migrating it to the current schema. A migration that cannot
         * complete leaves the file as it was and throws.
         *
         * One instance per process: a second one over the same directory would
         * put two writers on one database.
         */
        fun open(dataDirectory: File): TypviaStore {
            dataDirectory.mkdirs()
            return TypviaStore(
                // Opened **with** this device's key store. Without it the
                // bridge falls back to a stand-in that answers "unavailable"
                // to everything, and a device whose keys have nowhere to live
                // has no sync at all — no account, no pairing, no round.
                core = TypviaCore.openWithKeyKeeper(
                    dataDirectory.absolutePath,
                    AndroidKeyKeeper(File(dataDirectory, "keys")),
                ),
                dispatcher = Executors.newSingleThreadExecutor { runnable ->
                    Thread(runnable, "typvia-core").apply { isDaemon = true }
                }.asCoroutineDispatcher(),
            )
        }
    }
}

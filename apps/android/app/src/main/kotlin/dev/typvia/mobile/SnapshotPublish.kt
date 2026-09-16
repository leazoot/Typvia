// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ffi.TypviaStore
import java.io.File

/**
 * The document the keyboard reads.
 *
 * The app is the only writer — the input method runs in the same package and
 * reads this file, it never opens the database. "The bridge can write it" and
 * "somebody writes it" are two different facts, and the second one is this
 * file: without it the keyboard reads whatever was left on the device by an
 * older build, or nothing at all, and says so by showing an empty bench.
 */
object SnapshotPublish {
    /**
     * The directory the input method looks in, relative to the package's own
     * data directory. The name is the keyboard's, not this object's: it is
     * stated here because both halves have to agree, and only one of them can
     * be the source.
     */
    const val DIRECTORY = "keyboard-snapshot"

    /**
     * @return whether a document was written. A failure is reported rather
     *   than swallowed: the previous one stays where it is, and the keyboard
     *   goes on reading it.
     */
    suspend fun run(store: TypviaStore, dataDir: File): Boolean {
        val directory = File(dataDir, DIRECTORY)
        directory.mkdirs()
        return runCatching {
            store.perform { core -> core.writeSnapshot(directory.absolutePath) }
        }.isSuccess
    }
}

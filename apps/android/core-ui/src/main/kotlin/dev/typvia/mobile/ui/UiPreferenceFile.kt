// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import java.io.File

/**
 * The reader's two interface choices, as a file every process of this package
 * can read.
 *
 * The keyboard runs in its own process and is not inside the app's
 * composition, so it cannot be handed the reader's choice — it has to go and
 * find it. Multi-process `SharedPreferences` was deprecated for being
 * unreliable, so this takes the road the keyboard already travels: a small
 * file in the package's own data directory, written by the app and only ever
 * read by everyone else.
 *
 * The names live here because both halves have to agree and only one of them
 * can be the source.
 */
object UiPreferenceFile {
    /** Directory under the package's data dir, one file inside it. */
    const val DIRECTORY = "ui-preference"

    private const val FILE = "chosen"
    private const val TMP = "chosen.tmp"
    private const val LANGUAGE_KEY = "locale"
    private const val APPEARANCE_KEY = "theme"

    /** What the reader chose. Both defaults mean "whatever the phone says". */
    data class Chosen(
        val language: UiPreference.Language = UiPreference.Language.System,
        val appearance: UiPreference.Appearance = UiPreference.Appearance.System,
    )

    /**
     * Reads the file, or reports that the phone decides.
     *
     * Never absent, never a failure: a file that was never written, holds a
     * word this build does not know, or cannot be read at all all mean the
     * same thing to a surface about to draw itself — follow the system. The
     * alternative is an extension that refuses to render because it could not
     * find out what colour to be.
     */
    fun read(dataDir: File): Chosen {
        val lines = runCatching { File(File(dataDir, DIRECTORY), FILE).readLines() }
            .getOrElse { return Chosen() }
        val values = lines.mapNotNull { line ->
            val split = line.indexOf('=')
            if (split <= 0) null else line.substring(0, split) to line.substring(split + 1)
        }.toMap()
        return Chosen(
            language = UiPreference.Language.of(values[LANGUAGE_KEY]),
            appearance = UiPreference.Appearance.of(values[APPEARANCE_KEY]),
        )
    }

    /**
     * Replaces the file, whole or not at all.
     *
     * Temp file then rename, inside one directory: a reader whose phone dies
     * mid-write gets the previous choice back rather than half a word. The
     * same discipline the snapshot writer keeps, and for the same reason —
     * another process reads this without any way to know it was interrupted.
     *
     * @return whether the file now says what was asked for.
     */
    fun write(dataDir: File, chosen: Chosen): Boolean = runCatching {
        val directory = File(dataDir, DIRECTORY)
        directory.mkdirs()
        val tmp = File(directory, TMP)
        tmp.writeText(
            "$LANGUAGE_KEY=${chosen.language.stored}\n$APPEARANCE_KEY=${chosen.appearance.stored}\n",
        )
        tmp.renameTo(File(directory, FILE)).also { renamed ->
            if (!renamed) tmp.delete()
        }
    }.getOrDefault(false)
}

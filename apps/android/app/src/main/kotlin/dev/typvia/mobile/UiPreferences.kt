// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import android.content.Context
import android.content.SharedPreferences
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import dev.typvia.mobile.ui.UiPreference
import dev.typvia.mobile.ui.UiPreferenceFile
import java.io.File

/**
 * Where this device keeps the reader's two choices about how the app looks.
 *
 * Two strings, read before the window opens and written the moment they
 * change. Nothing about snippets is kept here and nothing here is ever synced:
 * these are facts about one screen, not about a library.
 *
 * The store is the platform's own rather than a database or a file of our own —
 * the values are two words, they must be readable synchronously on the way to
 * the first frame, and the framework already keeps that kind of thing.
 */
class UiPreferences internal constructor(
    private val store: SharedPreferences?,
    /**
     * The package's data directory, where the surfaces that run in their own
     * process look. Absent only where the platform would not name it, and
     * then this app is simply the only one that knows the reader's choice.
     */
    private val dataDir: File?,
) {
    private var languageState by mutableStateOf(
        UiPreference.Language.of(read(Keys.LANGUAGE)),
    )
    private var appearanceState by mutableStateOf(
        UiPreference.Appearance.of(read(Keys.APPEARANCE)),
    )

    var language: UiPreference.Language
        get() = languageState
        set(value) {
            languageState = value
            write(Keys.LANGUAGE, value.stored)
            publish()
        }

    var appearance: UiPreference.Appearance
        get() = appearanceState
        set(value) {
            appearanceState = value
            write(Keys.APPEARANCE, value.stored)
            publish()
        }

    /**
     * Puts the choice where the other processes look.
     *
     * The keyboard and the share sheet are not in this composition and cannot
     * be told; they read a file. Written on every change rather than on a
     * timer, because the reader expects the keyboard they pull up next to be
     * the one they just chose.
     */
    private fun publish() {
        dataDir?.let {
            UiPreferenceFile.write(
                it,
                UiPreferenceFile.Chosen(languageState, appearanceState),
            )
        }
    }

    private fun read(key: String): String? = runCatching { store?.getString(key, null) }.getOrNull()

    private fun write(key: String, value: String) {
        // A preference that could not be stored still applies to this run: the
        // reader chose it, and refusing to render their choice because it could
        // not be remembered would be losing twice instead of once.
        runCatching { store?.edit()?.putString(key, value)?.apply() }
    }

    /**
     * The keys are the ones the desktop and the other phone use, so the three
     * ends read as one product to whoever goes looking.
     */
    /**
     * Writes the file only where it does not already say this.
     *
     * A first run after an update has a stored preference and no file yet, and
     * a reader who never opens this app again would otherwise never get their
     * choice across to the keyboard. Reading two lines on the way to the first
     * frame is cheaper than the store this class already opened there.
     */
    private fun publishIfStale() {
        val directory = dataDir ?: return
        val wanted = UiPreferenceFile.Chosen(languageState, appearanceState)
        if (UiPreferenceFile.read(directory) != wanted) UiPreferenceFile.write(directory, wanted)
    }

    internal object Keys {
        const val LANGUAGE = "tv.ui.locale"
        const val APPEARANCE = "tv.ui.theme"
    }

    companion object {
        private const val FILE = "tv.ui"

        /**
         * Opens the store, or carries on without one.
         *
         * A device whose preference file cannot be opened is a device that
         * follows the system and forgets what it was told — which is the state
         * every device starts in anyway. It is not a reason to fail to start.
         */
        fun open(context: Context): UiPreferences = UiPreferences(
            runCatching { context.getSharedPreferences(FILE, Context.MODE_PRIVATE) }.getOrNull(),
            runCatching { context.dataDir }.getOrNull(),
        ).also { preferences -> preferences.publishIfStale() }
    }
}

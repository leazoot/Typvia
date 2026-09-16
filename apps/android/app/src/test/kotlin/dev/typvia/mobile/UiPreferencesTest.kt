// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ui.UiPreference
import dev.typvia.mobile.ui.UiPreferenceFile
import java.nio.file.Files
import org.junit.Assert.assertEquals
import org.junit.Test

/** Where this device keeps the reader's two choices about how the app looks. */
class UiPreferencesTest {
    @Test
    fun `the keys are the ones the other two ends use`() {
        assertEquals("tv.ui.locale", UiPreferences.Keys.LANGUAGE)
        assertEquals("tv.ui.theme", UiPreferences.Keys.APPEARANCE)
    }

    @Test
    fun `a device with no store to read follows the phone`() {
        val preferences = UiPreferences(store = null, dataDir = null)
        assertEquals(UiPreference.Language.System, preferences.language)
        assertEquals(UiPreference.Appearance.System, preferences.appearance)
    }

    /**
     * The keyboard and the share sheet are other processes: they are not in
     * this app's composition and cannot be told, so a choice that is not
     * written where they look never reaches them.
     */
    @Test
    fun `a choice is put where the other processes look`() {
        val dataDir = Files.createTempDirectory("typvia-prefs").toFile()
        val preferences = UiPreferences(store = null, dataDir = dataDir)

        preferences.language = UiPreference.Language.Zh
        preferences.appearance = UiPreference.Appearance.Dark

        assertEquals(
            UiPreferenceFile.Chosen(UiPreference.Language.Zh, UiPreference.Appearance.Dark),
            UiPreferenceFile.read(dataDir),
        )
    }

    @Test
    fun `a choice that cannot be remembered still applies to this run`() {
        // Losing the setting *and* refusing to render it would be losing twice.
        val preferences = UiPreferences(store = null, dataDir = null)
        preferences.language = UiPreference.Language.Zh
        preferences.appearance = UiPreference.Appearance.Dark
        assertEquals(UiPreference.Language.Zh, preferences.language)
        assertEquals(UiPreference.Appearance.Dark, preferences.appearance)
    }
}

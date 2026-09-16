// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The reader's two choices about how the product looks.
 *
 * What is checked here is the part that has to agree with the other two ends —
 * the words that go into storage — and the part that has to survive a store
 * holding something this build has never heard of.
 */
class AppearanceTest {
    @Test
    fun `the stored words are the ones the desktop and the other phone write`() {
        // Not decoration: the desktop reads these exact strings out of its own
        // store, and a phone that wrote "System" instead of "system" would be
        // a preference the product only half shares.
        assertEquals("system", UiPreference.Language.System.stored)
        assertEquals("en", UiPreference.Language.En.stored)
        assertEquals("zh", UiPreference.Language.Zh.stored)
        assertEquals("system", UiPreference.Appearance.System.stored)
        assertEquals("light", UiPreference.Appearance.Light.stored)
        assertEquals("dark", UiPreference.Appearance.Dark.stored)
    }

    @Test
    fun `every choice comes back as itself`() {
        for (choice in UiPreference.Language.entries) {
            assertEquals(choice, UiPreference.Language.of(choice.stored))
        }
        for (choice in UiPreference.Appearance.entries) {
            assertEquals(choice, UiPreference.Appearance.of(choice.stored))
        }
    }

    @Test
    fun `a word this build does not know is not guessed at`() {
        // Never set, and set by a version that knew more than this one, are the
        // same answer: follow the phone. Guessing would be a later version's
        // preference silently becoming a different one here.
        assertEquals(UiPreference.Language.System, UiPreference.Language.of(null))
        assertEquals(UiPreference.Language.System, UiPreference.Language.of(""))
        assertEquals(UiPreference.Language.System, UiPreference.Language.of("ja"))
        assertEquals(UiPreference.Language.System, UiPreference.Language.of("EN"))
        assertEquals(UiPreference.Appearance.System, UiPreference.Appearance.of(null))
        assertEquals(UiPreference.Appearance.System, UiPreference.Appearance.of("sepia"))
    }

    @Test
    fun `following the system reads the phone, and choosing ignores it`() {
        val chinesePhone = listOf("zh-Hans-CN", "en-US")
        assertEquals(UiLanguage.Zh, UiPreference.Language.System.resolved(chinesePhone))
        assertEquals(UiLanguage.En, UiPreference.Language.En.resolved(chinesePhone))
        assertEquals(UiLanguage.Zh, UiPreference.Language.Zh.resolved(listOf("de-DE")))
    }

    @Test
    fun `a chosen appearance holds in either phone`() {
        assertTrue(UiPreference.Appearance.System.isDark(systemIsDark = true))
        assertFalse(UiPreference.Appearance.System.isDark(systemIsDark = false))
        assertFalse(UiPreference.Appearance.Light.isDark(systemIsDark = true))
        assertTrue(UiPreference.Appearance.Dark.isDark(systemIsDark = false))
    }
}

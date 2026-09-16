// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.UiLanguage
import dev.typvia.mobile.ui.UiPreference
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** What the look-and-language chapter says. */
class AppearanceChapterTest {
    private val en = Translator(UiLanguage.En)
    private val zh = Translator(UiLanguage.Zh)

    /**
     * A language names itself. Somebody hunting for Chinese in an English
     * interface is scanning for 中文, and a row reading "Chinese" is the one
     * word they are not looking for.
     */
    @Test
    fun `each language is offered under its own name in either interface`() {
        assertEquals("English", AppearanceChapterCopy.name(UiPreference.Language.En, en))
        assertEquals("English", AppearanceChapterCopy.name(UiPreference.Language.En, zh))
        assertEquals("中文", AppearanceChapterCopy.name(UiPreference.Language.Zh, en))
        assertEquals("中文", AppearanceChapterCopy.name(UiPreference.Language.Zh, zh))
    }

    @Test
    fun `everything else on the page is written in both languages`() {
        assertNotEquals(
            AppearanceChapterCopy.name(UiPreference.Language.System, en),
            AppearanceChapterCopy.name(UiPreference.Language.System, zh),
        )
        for (option in UiPreference.Appearance.entries) {
            assertTrue(AppearanceChapterCopy.name(option, en).isNotBlank())
            assertNotEquals(
                AppearanceChapterCopy.name(option, en),
                AppearanceChapterCopy.name(option, zh),
            )
        }
    }

    /**
     * The contents page used to state "follows the system" whatever was true,
     * because on this platform nothing else could be. It is now a report, and
     * a report that does not change with what it reports is a decoration.
     */
    @Test
    fun `the contents page reports the choice rather than the platform`() {
        val summary = SettingsSummary(
            total = 0u,
            isSyncConfigured = false,
            deviceCount = 0,
            vaultPhase = VaultPhase.Absent,
        )
        assertEquals(
            "follows the phone",
            SettingsChapter.Appearance.value(summary, UiPreference.Language.System, en),
        )
        assertEquals(
            "English",
            SettingsChapter.Appearance.value(summary, UiPreference.Language.En, en),
        )
        assertEquals(
            "中文",
            SettingsChapter.Appearance.value(summary, UiPreference.Language.Zh, en),
        )
    }

    /**
     * Both choices offer the phone's own answer first. A page whose first
     * option is a commitment reads as one that requires a decision.
     */
    @Test
    fun `following the phone is the first option in both groups`() {
        assertEquals(UiPreference.Language.System, UiPreference.Language.entries.first())
        assertEquals(UiPreference.Appearance.System, UiPreference.Appearance.entries.first())
    }
}

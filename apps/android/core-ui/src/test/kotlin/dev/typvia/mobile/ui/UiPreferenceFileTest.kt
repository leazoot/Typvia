// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import java.io.File
import java.nio.file.Files
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The one road the reader's choice takes to a process that is not the app. */
class UiPreferenceFileTest {
    private fun dataDir(): File = Files.createTempDirectory("typvia-ui").toFile()

    @Test
    fun `what the app wrote is what the other process reads`() {
        val dataDir = dataDir()
        val chosen = UiPreferenceFile.Chosen(
            UiPreference.Language.Zh,
            UiPreference.Appearance.Dark,
        )

        assertTrue(UiPreferenceFile.write(dataDir, chosen))

        assertEquals(chosen, UiPreferenceFile.read(dataDir))
    }

    /**
     * Three ways to have nothing to read, one answer: follow the phone. An
     * extension that refused to draw because it could not find out what colour
     * to be would be a keyboard that does not come up.
     */
    @Test
    fun `every way of having nothing to read means the phone decides`() {
        val following = UiPreferenceFile.Chosen()

        // Never written.
        assertEquals(following, UiPreferenceFile.read(dataDir()))

        // Written with words this build does not know.
        val unknown = dataDir()
        File(unknown, UiPreferenceFile.DIRECTORY).mkdirs()
        File(File(unknown, UiPreferenceFile.DIRECTORY), "chosen")
            .writeText("locale=klingon\ntheme=sepia\n")
        assertEquals(following, UiPreferenceFile.read(unknown))

        // Not a directory this process may read at all.
        assertEquals(following, UiPreferenceFile.read(File("/proc/1/root/nowhere")))
    }

    /** A half-written line is not half a choice: the pair is read as a pair. */
    @Test
    fun `a file holding only one of the two still answers both`() {
        val dataDir = dataDir()
        File(dataDir, UiPreferenceFile.DIRECTORY).mkdirs()
        File(File(dataDir, UiPreferenceFile.DIRECTORY), "chosen").writeText("theme=light\n")

        val chosen = UiPreferenceFile.read(dataDir)

        assertEquals(UiPreference.Language.System, chosen.language)
        assertEquals(UiPreference.Appearance.Light, chosen.appearance)
    }

    /**
     * The reader changes their mind: the file says the new thing and nothing
     * of the old one is left beside it for another process to find.
     */
    @Test
    fun `a second choice replaces the first and leaves no temporary behind`() {
        val dataDir = dataDir()
        UiPreferenceFile.write(
            dataDir,
            UiPreferenceFile.Chosen(UiPreference.Language.Zh, UiPreference.Appearance.Dark),
        )

        UiPreferenceFile.write(
            dataDir,
            UiPreferenceFile.Chosen(UiPreference.Language.En, UiPreference.Appearance.System),
        )

        assertEquals(
            UiPreferenceFile.Chosen(UiPreference.Language.En, UiPreference.Appearance.System),
            UiPreferenceFile.read(dataDir),
        )
        assertEquals(
            listOf("chosen"),
            File(dataDir, UiPreferenceFile.DIRECTORY).list()?.sorted(),
        )
    }

    /** A directory that cannot be made is a write that says it did not happen. */
    @Test
    fun `a write that could not land says so`() {
        assertFalse(
            UiPreferenceFile.write(
                File("/proc/1/root/nowhere"),
                UiPreferenceFile.Chosen(UiPreference.Language.Zh),
            ),
        )
    }
}

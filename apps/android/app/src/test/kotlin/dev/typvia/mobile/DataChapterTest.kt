// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// The data chapter: what a file is taken to be, and what the page promises
// about the library after each outcome.

package dev.typvia.mobile

import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.UiLanguage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.typvia_mobile_ffi.BackupRestored
import uniffi.typvia_mobile_ffi.ImportReport
import uniffi.typvia_mobile_ffi.ImportSkipped
import uniffi.typvia_mobile_ffi.backupFormatMarker

class DataChapterTest {
    private val en = Translator(UiLanguage.En)

    /**
     * A sealed backup is recognised by the marker it actually carries, not by
     * being named `.json` — and the name is not trusted over the contents,
     * because a backup a reader renamed is still a backup.
     */
    @Test
    fun `a backup is known by its marker, whatever it is called`() {
        val sealed = """{"format":"${backupFormatMarker()}","version":1,"kdf":{},"sealed":"…"}"""

        assertEquals(PickedFile.Backup, PickedFile.of("library.json", sealed))
        assertEquals(PickedFile.Backup, PickedFile.of("whatever.txt", sealed))
    }

    /**
     * Three products write `.json`, and importing one as another turns a
     * library into gibberish. Where the product cannot know, it asks.
     */
    @Test
    fun `a json that is not a backup is asked about rather than guessed at`() {
        assertEquals(PickedFile.AskWhichJson, PickedFile.of("snippets.json", "[{\"body\":\"x\"}]"))
        assertEquals(3, PickedFile.JSON_KINDS.size)
    }

    @Test
    fun `an extension that says what it is goes straight through`() {
        assertEquals(PickedFile.Known("markdown"), PickedFile.of("notes.MD", "# hi"))
        assertEquals(PickedFile.Known("csv"), PickedFile.of("rows.csv", "a,b"))
        assertEquals(PickedFile.Unreadable, PickedFile.of("photo.png", "�"))
    }

    /**
     * Conflicts are named, not counted: a reader told "3 conflicts" cannot go
     * and look at the three they already have.
     */
    @Test
    fun `an import report names the triggers that stayed out`() {
        val report = ImportReport(
            imported = 2u,
            conflicts = listOf(";ssh", ";deploy"),
            skipped = listOf(ImportSkipped(label = "row 4", reason = "empty")),
        )

        val said = DataChapterCopy.imported(report, en)

        assertTrue(said.startsWith("2 snippets came in."))
        assertTrue(said.contains(";ssh, ;deploy"))
        assertTrue("what could not be read is counted, not quoted", said.contains("1 entry could not be read"))
    }

    /** English has a plural and one snippet is not "1 snippets". */
    @Test
    fun `one snippet is said as one`() {
        val one = ImportReport(imported = 1u, conflicts = emptyList(), skipped = emptyList())

        assertEquals("1 snippet came in.", DataChapterCopy.imported(one, en))
    }

    /** Every noun in the restore line decides its own plural. */
    @Test
    fun `a restore of one of each thing says each of them in the singular`() {
        val one = BackupRestored(
            snippets = 1u, folders = 1u, tags = 1u, versions = 1u, vaultRestored = false,
        )

        assertTrue(DataChapterCopy.restored(one, en).startsWith("Back: 1 snippet, 1 folder, 1 tag."))
    }

    /**
     * A restore either brought the vault or it did not, and silence about it
     * would leave the reader wondering where their secrets went.
     */
    @Test
    fun `a restore says whether the vault came with it`() {
        val withVault = BackupRestored(
            snippets = 12u, folders = 2u, tags = 3u, versions = 40u, vaultRestored = true,
        )
        assertTrue(DataChapterCopy.restored(withVault, en).contains("The vault came with it"))

        val without = withVault.copy(vaultRestored = false)
        assertTrue(DataChapterCopy.restored(without, en).contains("no vault in that backup"))
    }

    /**
     * This is the page where a reader's whole library is at stake, so every
     * refusal on it ends by saying the library is untouched.
     */
    @Test
    fun `every refusal ends by saying the library is unchanged`() {
        for (refusal in DataRefusal.entries) {
            val said = DataChapterCopy.sentence(refusal, en)

            assertTrue(
                "$refusal does not say what is still true",
                said.endsWith(DataChapterCopy.nothingChanged(en)),
            )
        }
    }
}

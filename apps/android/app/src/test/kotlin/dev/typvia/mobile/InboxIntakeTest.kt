// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ffi.TypviaStore
import dev.typvia.share.ShareInbox
import java.io.File
import java.nio.file.Files
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * The other half of the share sheet.
 *
 * The sheet writes a document and never opens the database; without this the
 * text it accepted would arrive nowhere — a sheet that says "saved" and means
 * nothing. The documents here are written by the real writer, so the two halves
 * are tested against each other rather than against my idea of the format.
 */
class InboxIntakeTest {
    private lateinit var directory: File
    private lateinit var store: TypviaStore

    @Before
    fun open() {
        directory = Files.createTempDirectory("typvia-inbox").toFile()
        store = TypviaStore.open(File(directory, "db"))
    }

    @After
    fun close() {
        directory.deleteRecursively()
    }

    @Test
    fun `what was shared lands in the library and the document is cleared`() = runBlocking {
        assertTrue(ShareInbox.write(directory, 1_700_000_000_000, "Rollback", "kubectl rollout undo"))

        val filed = InboxIntake.run(store, directory)

        assertEquals(1, filed)
        assertEquals(1u, store.perform { it.snippetCount("all", null, null) })
        assertEquals(0, File(directory, InboxIntake.DIRECTORY).listFiles()?.size ?: 0)
    }

    @Test
    fun `two shares before the app was opened both land`() = runBlocking {
        ShareInbox.write(directory, 1, "One", "first")
        ShareInbox.write(directory, 2, "Two", "second")

        assertEquals(2, InboxIntake.run(store, directory))
        assertEquals(2u, store.perform { it.snippetCount("all", null, null) })
    }

    /// An untitled share is named by the shared layer, the same way an
    /// untitled snippet typed into the editor is.
    @Test
    fun `an untitled share is filed under its first line`() = runBlocking {
        ShareInbox.write(directory, 1, null, "kubectl get pods\nall of them")

        assertEquals(1, InboxIntake.run(store, directory))

        // Asked of `all`, not `recent`: a snippet nobody has used yet is not
        // in the recall order, which is a fact about that view rather than
        // about whether the snippet arrived.
        val listed = store.perform { it.snippetListPage("all", null, null, 5u, 0u) }
        assertEquals("kubectl get pods", listed.single().title)
    }

    @Test
    fun `an empty inbox is not an event`() = runBlocking {
        assertEquals(0, InboxIntake.run(store, directory))
    }

    /// A document that cannot be read is left where it is rather than deleted:
    /// the alternative is throwing away something the reader watched the sheet
    /// accept.
    @Test
    fun `an unreadable document is kept rather than dropped`() = runBlocking {
        val inbox = File(directory, InboxIntake.DIRECTORY)
        inbox.mkdirs()
        File(inbox, "share-broken.json").writeText("{not json at all")

        assertEquals(0, InboxIntake.run(store, directory))
        assertEquals(1, inbox.listFiles()?.size ?: 0)
    }

    /// Round-tripped through the real writer rather than through a string I
    /// typed: what is being checked is that the two halves agree, and a
    /// hand-written fixture only checks that I agree with myself.
    @Test
    fun `the parser reads what the writer wrote, escapes and all`() = runBlocking {
        ShareInbox.write(directory, 1_700_000_000_000, "A \"quoted\" name", "one\ntwo\tthree")
        val document = File(directory, InboxIntake.DIRECTORY).listFiles()!!.single()

        val item = SharedItem.parse(document.readText())

        assertEquals("A \"quoted\" name", item?.title)
        assertEquals("one\ntwo\tthree", item?.text)
        assertEquals(1_700_000_000_000, item?.sharedAt)
    }

    @Test
    fun `a document from a schema this build does not know is left alone`() {
        assertNull(SharedItem.parse("""{"schema_version":2,"shared_at":1,"text":"hi"}"""))
    }
}

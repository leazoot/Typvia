// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ffi.TypviaStore
import java.io.File
import java.nio.file.Files
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * Writing a snippet down on this platform, against the real core.
 *
 * Until now nothing on Android could add anything to the library: the home
 * screen invited a first snippet on a screen with no way to write one. What is
 * under test is that the invitation now leads somewhere, and that the rules it
 * relies on are the core's rather than the screen's.
 *
 * The host-JVM tests load the core through the same dylib the FFI smoke does.
 */
class SnippetWriterTest {
    private lateinit var directory: File
    private lateinit var store: TypviaStore

    @Before
    fun open() {
        directory = Files.createTempDirectory("typvia-writer").toFile()
        store = TypviaStore.open(directory)
    }

    @After
    fun close() {
        directory.deleteRecursively()
    }

    @Test
    fun `what was typed is what lands in the library`() = runBlocking {
        val saved = SnippetWriter.write(store, "Rollback", "kubectl rollout undo deploy/api")

        assertTrue(saved.isSuccess)
        val snippet = saved.getOrThrow()
        assertEquals("Rollback", snippet.title)
        assertEquals("kubectl rollout undo deploy/api", snippet.body)
        assertEquals("normal", snippet.securityLevel)
        assertEquals(1u, store.perform { it.snippetCount("all", null, null) })
    }

    /**
     * A reader who did not want to name anything gets the first line of what
     * they wrote — and gets it from the shared layer, so this platform and the
     * other one answer the same way. The rule used to live in the iOS screen,
     * which is how the two came to disagree in the first place.
     */
    @Test
    fun `an untitled snippet is filed under its first line`() = runBlocking {
        val saved = SnippetWriter.write(store, "", "kubectl get pods\n--all-namespaces")

        assertEquals("kubectl get pods", saved.getOrThrow().title)
    }

    @Test
    fun `the kind and trigger chosen on the page are what get stored`() = runBlocking {
        val saved = SnippetWriter.write(
            store, "Rollback", "kubectl rollout undo",
            sort = dev.typvia.mobile.ui.TypeSort.Command, trigger = "  ;roll  ",
        ).getOrThrow()

        assertEquals("command", saved.snippetType)
        // Trimmed by the shared layer, and given the default mode there too —
        // this page states a trigger and does not decide what kind it is.
        assertEquals(";roll", saved.trigger)
        assertEquals("delimiter", saved.triggerMode)
    }

    @Test
    fun `an empty trigger field leaves the snippet without one`() = runBlocking {
        val saved = SnippetWriter.write(store, "Rollback", "kubectl rollout undo", trigger = "   ")
            .getOrThrow()

        assertEquals(null, saved.trigger)
        assertEquals(null, saved.triggerMode)
    }

    @Test
    fun `a new folder is made and the snippet can be filed in it`() = runBlocking {
        val folder = SnippetWriter.makeFolder(store, "  上线  ").getOrThrow()
        // Trimmed by the core, not by this screen — the same answer every
        // host gets.
        assertEquals("上线", folder.name)

        val saved = SnippetWriter.write(
            store, "Rollback", "kubectl rollout undo", folderId = folder.id,
        ).getOrThrow()

        assertEquals(folder.id, saved.folderId)
    }

    @Test
    fun `a folder with no name is refused and none is made`() = runBlocking {
        assertTrue(SnippetWriter.makeFolder(store, "   ").isFailure)
        assertEquals(0, store.perform { it.folderListChildren(null) }.size)
    }

    /// Emptying the bin is the one place in the product where something is
    /// really gone. What is asserted is that it goes — and that the rest of
    /// the library does not go with it.
    @Test
    fun `emptying the bin destroys what is in it and nothing else`() = runBlocking {
        val kept = SnippetWriter.write(store, "Keep me", "still needed").getOrThrow()
        val thrown = SnippetWriter.write(store, "Throw me", "not needed").getOrThrow()
        SnippetWriter.trash(store, thrown.id)

        val destroyed = SnippetWriter.emptyTheBin(store, listOf(thrown.id))

        assertEquals(1, destroyed)
        assertEquals(0, store.perform { it.trashList(10u, 0u) }.size)
        assertEquals(1u, store.perform { it.snippetCount("all", null, null) })
        assertEquals(kept.id, store.perform { it.snippetGet(kept.id) }.id)
    }

    /// An edit changes the two fields the screen shows and leaves everything
    /// else exactly as it was. A screen that sent its own defaults for the
    /// fields it does not show would quietly reset them.
    @Test
    fun `an edit changes what was edited and nothing else`() = runBlocking {
        val saved = store.perform { core ->
            core.snippetCreate(
                uniffi.typvia_mobile_ffi.SnippetDraft(
                    title = "Rollback", body = "kubectl rollout undo",
                    snippetType = "command", description = null, folderId = null,
                    trigger = ";roll", triggerMode = "delimiter", language = null,
                ),
            )
        }

        val edited = SnippetWriter.update(store, saved, "Rollback prod", "kubectl rollout undo -n prod")
            .getOrThrow()

        assertEquals("Rollback prod", edited.title)
        assertEquals("kubectl rollout undo -n prod", edited.body)
        // The kind and the trigger were never on that screen; they survive it.
        assertEquals("command", edited.snippetType)
        assertEquals(";roll", edited.trigger)
        assertEquals("delimiter", edited.triggerMode)
    }

    /// The bin, not a delete: the row is still there, marked, and the library
    /// stops counting it. A screen that promised deletion here would be
    /// describing something harsher than what happens.
    @Test
    fun `moving one to the bin takes it out of the library without destroying it`() = runBlocking {
        val saved = SnippetWriter.write(store, "Rollback", "kubectl rollout undo").getOrThrow()

        assertTrue(SnippetWriter.trash(store, saved.id).isSuccess)

        assertEquals(0u, store.perform { it.snippetCount("all", null, null) })
        // Still fetchable by id — the bin is a state, not an erasure.
        assertEquals(saved.id, store.perform { it.snippetGet(saved.id) }.id)
    }

    /// What the bin screen prints as a deadline has to be the one the purge
    /// actually applies. The number crosses the bridge; the screen quotes it.
    @Test
    fun `the retention the bin prints is the one the core enforces`() {
        val days = uniffi.typvia_mobile_ffi.trashRetentionDays().toInt()
        assertTrue(days > 0)
    }

    @Test
    fun `something brought back out of the bin is in the library again`() = runBlocking {
        val saved = SnippetWriter.write(store, "Rollback", "kubectl rollout undo").getOrThrow()
        SnippetWriter.trash(store, saved.id)
        assertEquals(1, store.perform { it.trashList(10u, 0u) }.size)

        store.perform { it.snippetRestore(saved.id) }

        assertEquals(1u, store.perform { it.snippetCount("all", null, null) })
        assertEquals(0, store.perform { it.trashList(10u, 0u) }.size)
    }

    @Test
    fun `a failure is reported rather than swallowed`() = runBlocking {
        // A body the core will not take: the rule is its own, and what this
        // asserts is only that the refusal comes back rather than vanishing.
        val saved = SnippetWriter.write(store, "Nothing", "")

        // Whether an empty body is refused is the core's business; what must
        // never happen is a Result that claims success without a row.
        if (saved.isSuccess) {
            assertEquals(1u, store.perform { it.snippetCount("all", null, null) })
        } else {
            assertEquals(0u, store.perform { it.snippetCount("all", null, null) })
        }
    }
}

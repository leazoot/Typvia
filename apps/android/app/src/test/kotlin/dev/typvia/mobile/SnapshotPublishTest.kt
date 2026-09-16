// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// The document the keyboard reads, written against the real core.
//
// What is under test is the half nobody notices until it is missing: that a
// snippet saved in the app is *in* the document before the reader reaches for
// the keyboard, rather than one app-switch later.

package dev.typvia.mobile

import java.io.File
import java.nio.file.Files
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.typvia_mobile_ffi.snapshotEntries

class SnapshotPublishTest {
    private lateinit var directory: File
    private lateinit var store: dev.typvia.mobile.ffi.TypviaStore

    @Before
    fun open() {
        directory = Files.createTempDirectory("typvia-snapshot").toFile()
        store = dev.typvia.mobile.ffi.TypviaStore.open(directory)
    }

    @After
    fun close() {
        directory.deleteRecursively()
    }

    /** Where the keyboard looks. Both halves have to name the same place. */
    private fun document(): File =
        File(File(directory, SnapshotPublish.DIRECTORY), "snapshot.json")

    private fun titlesTheKeyboardCanSee(): List<String> =
        snapshotEntries(document().readText()).map { it.title }

    @Test
    fun `a snippet saved in the app is in the document the keyboard reads`() = runBlocking {
        SnippetWriter.write(store, "Rollback", "kubectl rollout undo deploy/api")

        assertTrue(SnapshotPublish.run(store, directory))

        assertEquals(listOf("Rollback"), titlesTheKeyboardCanSee())
    }

    @Test
    fun `a second save is in it too, without the app having been left`() = runBlocking {
        SnippetWriter.write(store, "Rollback", "kubectl rollout undo deploy/api")
        SnapshotPublish.run(store, directory)
        SnippetWriter.write(store, "Docker logs", "docker logs -f app")

        SnapshotPublish.run(store, directory)

        assertEquals(
            setOf("Rollback", "Docker logs"),
            titlesTheKeyboardCanSee().toSet(),
        )
    }

    @Test
    fun `what was thrown away is no longer in it`() = runBlocking {
        val saved = SnippetWriter.write(store, "Rollback", "kubectl rollout undo deploy/api")
        SnapshotPublish.run(store, directory)

        SnippetWriter.trash(store, saved.getOrThrow().id)
        SnapshotPublish.run(store, directory)

        assertTrue(titlesTheKeyboardCanSee().isEmpty())
    }

    /**
     * The keyboard reads a file, and a half-written file is worse than an old
     * one. What lands must always be a document the reader can be shown.
     */
    @Test
    fun `the document is always whole`() = runBlocking {
        SnippetWriter.write(store, "Rollback", "kubectl rollout undo deploy/api")
        SnapshotPublish.run(store, directory)
        val first = document().readText()

        SnippetWriter.write(store, "Docker logs", "docker logs -f app")
        SnapshotPublish.run(store, directory)
        val second = document().readText()

        assertFalse("the second write must replace the first", first == second)
        assertEquals(2, snapshotEntries(second).size)
    }
}

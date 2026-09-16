// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Writing a secret on this platform, against the real core.
//
// The path did not exist here: the kind picker offered seven marks because
// there was no vault to write the eighth through. What is under test is that
// the eighth now lands in the vault — and that the ordinary door stays shut to
// it, so the guarantee does not rest on a screen routing correctly.

package dev.typvia.mobile

import dev.typvia.mobile.ffi.TypviaStore
import dev.typvia.mobile.ui.TypeSort
import java.io.File
import java.nio.file.Files
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class SecretWritingTest {
    private lateinit var directory: File
    private lateinit var store: TypviaStore

    /** Deliberately fake, and shaped so a byte scan can find it if it leaks. */
    private val secretBody = "AKIA_FAKE_NOT_A_SECRET_deploy_value"
    private val master = "correct horse battery"

    @Before
    fun open() {
        directory = Files.createTempDirectory("typvia-secret").toFile()
        store = TypviaStore.open(directory)
    }

    @After
    fun close() {
        directory.deleteRecursively()
    }

    @Test
    fun `a secret written with the vault open lands in it and hands back no body`() = runBlocking {
        VaultKeeper.make(store, master)

        val written = SnippetWriter.writeSecret(store, "Deploy key", secretBody)

        assertTrue(written.isSuccess)
        val secret = written.getOrThrow()
        assertEquals("sensitive", secret.securityLevel)
        assertNull("a secret's row carries no body", secret.body)
        assertEquals(listOf("Deploy key"), VaultKeeper.list(store)?.map { it.title })
    }

    @Test
    fun `with no vault the write is refused rather than filed somewhere else`() = runBlocking {
        val written = SnippetWriter.writeSecret(store, "Deploy key", secretBody)

        assertTrue(written.isFailure)
        assertEquals(0u, store.perform { it.snippetCount("all", null, null) })
    }

    @Test
    fun `the held words survive the vault being asked for`() = runBlocking {
        val held = PendingSecret("Deploy key", secretBody, trigger = ";dk", folderId = null)
        VaultKeeper.make(store, master)

        var settled = false
        finishSecret(store, held) { settled = it }

        assertTrue(settled)
        val secret = store.perform { it.vaultList(10u, 0u) }.single()
        assertEquals("Deploy key", secret.title)
        assertEquals(";dk", secret.trigger)
    }

    /**
     * The core refuses an unnamed secret, and it is right to: a title is
     * searchable, so the untitled-snippet rule — file it under its first line —
     * would put the secret itself in the index. The screen has to ask for a
     * name rather than borrow one.
     */
    @Test
    fun `an unnamed secret is refused rather than named after its own words`() = runBlocking {
        VaultKeeper.make(store, master)

        val refused = SnippetWriter.writeSecret(store, "", secretBody)

        assertTrue(refused.isFailure)
        assertTrue(VaultKeeper.list(store)!!.isEmpty())
        // And nothing of it is searchable under the name it did not get.
        assertTrue(store.perform { it.searchAll(secretBody, 20u) }.isEmpty())
    }

    // MARK: Taking one out, and putting one in

    @Test
    fun `taking one out gives back exactly what went in`() = runBlocking {
        VaultKeeper.make(store, master)
        val secret = SnippetWriter.writeSecret(store, "Deploy key", secretBody).getOrThrow()

        val out = VaultKeeper.takeOut(store, secret.id)

        assertEquals(secretBody, out.getOrThrow())
    }

    @Test
    fun `a shut vault gives nothing back`() = runBlocking {
        VaultKeeper.make(store, master)
        val secret = SnippetWriter.writeSecret(store, "Deploy key", secretBody).getOrThrow()
        VaultKeeper.shut(store)

        assertTrue(VaultKeeper.takeOut(store, secret.id).isFailure)
    }

    /**
     * Moving an ordinary snippet in is the core's own act. What matters most
     * is the half nobody sees: the words it used to be must stop being
     * searchable, or the reader has moved a copy and left the original.
     */
    @Test
    fun `putting a saved snippet in takes its cleartext past with it`() = runBlocking {
        VaultKeeper.make(store, master)
        val saved = SnippetWriter.write(store, "Cluster login", secretBody).getOrThrow()
        assertFalse(store.perform { it.searchSnippets(secretBody, 20u, 0u) }.isEmpty())

        assertTrue(VaultKeeper.putIn(store, saved.id).isSuccess)

        val moved = store.perform { it.snippetGet(saved.id) }
        assertEquals("sensitive", moved?.securityLevel)
        assertNull("its row hands back no body now", moved?.body)
        assertTrue(
            "what it used to be must not stay searchable",
            store.perform { it.searchSnippets(secretBody, 20u, 0u) }.isEmpty(),
        )
    }

    @Test
    fun `the words come out for a bounded time, not indefinitely`() {
        // The page covers them again on its own. A number that is zero or
        // absent would mean a key stays on screen until something else
        // happens to the phone.
        assertTrue(VaultKeeper.TAKEN_OUT_SECONDS in 1..60)
    }

    // MARK: Red lines

    @Test
    fun `the ordinary door refuses to file something as a secret`() = runBlocking {
        VaultKeeper.make(store, master)

        val refused = SnippetWriter.write(store, "Deploy key", secretBody, TypeSort.Secret)

        assertTrue("the core refuses the secret kind at the ordinary door", refused.isFailure)
    }

    @Test
    fun `what was written as a secret is nowhere in the library's files`() = runBlocking {
        VaultKeeper.make(store, master)
        SnippetWriter.writeSecret(store, "Deploy key", secretBody)

        assertTrue(store.perform { it.searchSnippets(secretBody, 20u, 0u) }.isEmpty())
        assertTrue(store.perform { it.searchAll(secretBody, 20u) }.isEmpty())

        val needle = secretBody.toByteArray()
        for (file in directory.walkTopDown().filter { it.isFile }) {
            assertFalse(
                "${file.name} holds the plaintext",
                file.readBytes().containsSlice(needle),
            )
        }
    }
}

private fun ByteArray.containsSlice(other: ByteArray): Boolean {
    if (other.isEmpty() || other.size > size) return false
    outer@ for (start in 0..size - other.size) {
        for (offset in other.indices) {
            if (this[start + offset] != other[offset]) continue@outer
        }
        return true
    }
    return false
}

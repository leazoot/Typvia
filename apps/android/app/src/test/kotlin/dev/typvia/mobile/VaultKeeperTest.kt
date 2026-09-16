// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Making and opening the vault on this platform, against the real core.
//
// Until now the room could draw all three of its faces and reach none of
// them: this device could not make a vault and could not open one. What is
// under test is the way in, and that every rule it leans on is the core's.

package dev.typvia.mobile

import dev.typvia.mobile.ffi.TypviaStore
import java.io.File
import java.nio.file.Files
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class VaultKeeperTest {
    private lateinit var directory: File
    private lateinit var store: TypviaStore

    /** Deliberately fake, and shaped so a byte scan can find it if it leaks. */
    private val secretBody = "AKIA_FAKE_NOT_A_SECRET_deploy_value"
    private val master = "correct horse battery"

    @Before
    fun open() {
        directory = Files.createTempDirectory("typvia-vault").toFile()
        store = TypviaStore.open(directory)
    }

    @After
    fun close() {
        directory.deleteRecursively()
    }

    @Test
    fun `a device with no vault says so, and making one opens it`() = runBlocking {
        assertEquals(VaultPhase.Absent, VaultPhase.of(VaultKeeper.status(store)))

        val made = VaultKeeper.make(store, master)

        assertTrue(made.isSuccess)
        assertEquals(VaultPhase.Open, VaultPhase.of(made.getOrThrow()))
    }

    @Test
    fun `the shortest master password is the one the vault enforces`() = runBlocking {
        val shortest = VaultKeeper.shortestPassword()
        assertTrue("the floor must be a real number", shortest > 0)

        val tooShort = VaultKeeper.make(store, "x".repeat(shortest - 1))

        assertTrue(tooShort.isFailure)
        assertEquals(
            "the refusal is the vault's, not the screen's guess",
            VaultRefusal.NotAPassword,
            VaultRefusal.of(tooShort.exceptionOrNull()!!),
        )
        assertEquals(VaultPhase.Absent, VaultPhase.of(VaultKeeper.status(store)))

        assertTrue(VaultKeeper.make(store, "x".repeat(shortest)).isSuccess)
    }

    @Test
    fun `two different typings are caught before anything is sent`() {
        assertFalse(VaultKeeper.typedTwiceMatches(master, "correct horse"))
        assertTrue(VaultKeeper.typedTwiceMatches(master, master))
    }

    @Test
    fun `the wrong password leaves the vault shut and says nothing about it`() = runBlocking {
        VaultKeeper.make(store, master)
        VaultKeeper.shut(store)

        val refused = VaultKeeper.open(store, "not the master password")

        assertTrue(refused.isFailure)
        assertEquals(VaultRefusal.DidNotOpen, VaultRefusal.of(refused.exceptionOrNull()!!))
        assertEquals(VaultPhase.Shut, VaultPhase.of(VaultKeeper.status(store)))
    }

    @Test
    fun `shutting it and opening it again is the same vault`() = runBlocking {
        VaultKeeper.make(store, master)
        val secret = store.perform {
            it.vaultCreateSecret(
                uniffi.typvia_mobile_ffi.SnippetDraft(
                    title = "Deploy key", body = secretBody, snippetType = "sensitive",
                    description = null, folderId = null, trigger = null, triggerMode = null,
                    language = null,
                ),
            )
        }

        VaultKeeper.shut(store)
        assertEquals(VaultPhase.Shut, VaultPhase.of(VaultKeeper.status(store)))

        assertTrue(VaultKeeper.open(store, master).isSuccess)
        val listed = VaultKeeper.list(store)
        assertNotNull(listed)
        assertEquals(listOf(secret.title), listed!!.map { it.title })
    }

    // MARK: Red lines

    /**
     * A shut vault still knows the names — that is deliberate, and it is why
     * a locked secret can be found in the library at all. What it will not do
     * is hand over what is inside one. The room's shut face shows no list, but
     * that is the room's manners; **this** is the guarantee.
     */
    @Test
    fun `a shut vault hands out no bodies`() = runBlocking {
        VaultKeeper.make(store, master)
        val secret = store.perform {
            it.vaultCreateSecret(
                uniffi.typvia_mobile_ffi.SnippetDraft(
                    title = "Deploy key", body = secretBody, snippetType = "sensitive",
                    description = null, folderId = null, trigger = null, triggerMode = null,
                    language = null,
                ),
            )
        }
        assertEquals(secretBody, store.perform { it.vaultReveal(secret.id) })

        VaultKeeper.shut(store)

        val refused = runCatching { store.perform { it.vaultReveal(secret.id) } }
        assertTrue("a shut vault reveals nothing", refused.isFailure)
        assertEquals(VaultRefusal.DidNotOpen, VaultRefusal.of(refused.exceptionOrNull()!!))
    }

    /**
     * Byte level, not field level. What was written through the vault must not
     * be anywhere in the files the library is made of — the write-ahead log
     * included, which is where a plaintext insert still shows up after the row
     * itself has been replaced.
     */
    @Test
    fun `what went into the vault is nowhere in the library's files`() = runBlocking {
        VaultKeeper.make(store, master)
        store.perform {
            it.vaultCreateSecret(
                uniffi.typvia_mobile_ffi.SnippetDraft(
                    title = "Deploy key", body = secretBody, snippetType = "sensitive",
                    description = null, folderId = null, trigger = null, triggerMode = null,
                    language = null,
                ),
            )
        }

        val hits = store.perform { it.searchSnippets(secretBody, 20u, 0u) }
        assertTrue("a secret is not in the ordinary index", hits.isEmpty())

        val needle = secretBody.toByteArray()
        for (file in directory.walkTopDown().filter { it.isFile }) {
            val bytes = file.readBytes()
            assertFalse("${file.name} holds the plaintext", bytes.indexOfSlice(needle) >= 0)
        }
    }
}

/** First index of [other] in this array, or -1. */
private fun ByteArray.indexOfSlice(other: ByteArray): Int {
    if (other.isEmpty() || other.size > size) return -1
    outer@ for (start in 0..size - other.size) {
        for (offset in other.indices) {
            if (this[start + offset] != other[offset]) continue@outer
        }
        return start
    }
    return -1
}

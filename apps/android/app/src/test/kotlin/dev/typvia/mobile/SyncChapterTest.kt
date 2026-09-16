// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// What the sync chapter says. The rule under test is the one this page exists
// to keep: it reports, it never reassures — and where a fact is missing it is
// missing rather than filled in with an optimistic default.

package dev.typvia.mobile

import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.UiLanguage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.typvia_mobile_ffi.SyncDevice
import uniffi.typvia_mobile_ffi.SyncRound
import uniffi.typvia_mobile_ffi.SyncStatus

class SyncChapterTest {
    private val en = Translator(UiLanguage.En)

    private fun status(
        available: Boolean = true,
        configured: Boolean = true,
        enabled: Boolean = true,
        backlog: ULong = 0uL,
    ) = SyncStatus(
        available = available,
        configured = configured,
        enabled = enabled,
        serverUrl = null,
        accountId = null,
        deviceId = "d1",
        deviceName = "This phone",
        keyGeneration = 1u,
        pendingBacklog = backlog,
        conflictCount = 0u,
        lastSyncAt = null,
        lastFailure = null,
        vaultReady = false,
        vaultUnlocked = false,
        recoveryExportedAt = null,
        recoveryCatchupPending = false,
        transportKind = "server",
    )

    /**
     * "Could not read" is not "not set up". Telling a reader to go and set up
     * sync because a read failed sends them looking for a problem that is not
     * theirs.
     */
    @Test
    fun `a status that could not be read says so rather than saying not set up`() {
        val said = SyncChapterCopy.headline(null, en)

        assertTrue(said.contains("could not be read"))
        assertTrue("what is still good comes first", said.startsWith("Everything is on this device"))
    }

    /**
     * A phone with no gated key store cannot hold a sync key at all. Saying
     * "not set up" there invites the reader to set up something this device
     * cannot do.
     */
    @Test
    fun `a device that cannot keep a key says that rather than offering setup`() {
        val said = SyncChapterCopy.headline(status(available = false), en)

        assertTrue(said.contains("cannot keep a sync key"))
    }

    @Test
    fun `a queue is stated as a number and as what happens next`() {
        val said = SyncChapterCopy.headline(status(backlog = 7uL), en)

        assertTrue(said.contains("7"))
        assertTrue("the reader is told they need do nothing", said.contains("when the network comes back"))
    }

    /**
     * One change queued is a sentence of its own: the verb agrees as well as
     * the noun, and this line used to read "1 changes are queued".
     */
    @Test
    fun `one queued change is said in the singular, verb and all`() {
        val said = SyncChapterCopy.headline(status(backlog = 1uL), en)

        assertEquals("1 change is queued. It goes out when the network comes back.", said)
    }

    @Test
    fun `switched off says the device still works`() {
        assertTrue(SyncChapterCopy.headline(status(enabled = false), en).contains("works exactly as it did"))
    }

    @Test
    fun `a round is reported as what it did, both directions`() {
        val round = SyncRound(
            pushed = 3u, applied = 1u, merged = 0u, conflictCopies = 0u,
            parked = 0u, skipped = 0u, pendingBacklog = 0uL, at = 0,
        )

        assertEquals("Sent 3, received 1.", SyncChapterCopy.round(round, en))
        assertNull("a round that never ran is not reported as zero", SyncChapterCopy.round(null, en))
    }

    /**
     * A way on is offered only where it can be taken. The two noes that are
     * easy to get wrong: a status that could not be read makes no claim about
     * what is possible, and a device with no gated key store has nothing to
     * hold a sync key in.
     */
    @Test
    fun `joining is offered only to a device that could actually join`() {
        assertTrue(SyncChapterCopy.offersJoining(status(configured = false)))
        assertFalse("a read that failed is not an invitation", SyncChapterCopy.offersJoining(null))
        assertFalse(SyncChapterCopy.offersJoining(status(available = false, configured = false)))
        assertFalse(
            "a device with an account is not looking for one",
            SyncChapterCopy.offersJoining(status(configured = true)),
        )
    }

    /**
     * "Verified" is the ordinary case. Printing it on every row turns the word
     * into decoration, and decoration is the last thing a trust marker may be.
     */
    @Test
    fun `a device row names what is unusual about it, not what is normal`() {
        val ordinary = SyncDevice(
            deviceId = "d2", name = "MacBook", platform = "macos", createdAt = 0,
            revokedAt = null, verified = true, isThisDevice = false, isRoot = false,
        )
        assertEquals("macos", SyncChapterCopy.deviceMeta(ordinary, en))

        val odd = ordinary.copy(verified = false, isThisDevice = true)
        assertEquals("this one · macos · unverified", SyncChapterCopy.deviceMeta(odd, en))
    }
}

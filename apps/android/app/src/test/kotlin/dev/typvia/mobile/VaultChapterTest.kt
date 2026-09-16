// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// What the vault's own settings chapter says. Every sentence on that page is
// about this device's vault, so every one of them has to be true of it.

package dev.typvia.mobile

import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.UiLanguage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.typvia_mobile_ffi.VaultStatus

class VaultChapterTest {
    private val en = Translator(UiLanguage.En)
    private val zh = Translator(UiLanguage.Zh)

    private fun status(idleMs: Long) = VaultStatus(
        initialized = true,
        unlocked = true,
        unlockedAt = null,
        lastActivityAt = null,
        idleTimeoutMs = idleMs,
    )

    @Test
    fun `the window it re-locks after is the core's number, in minutes`() {
        assertEquals("5 minutes", VaultChapterCopy.idleWindow(status(300_000), en))
    }

    /** A one-minute window is "1 minute"; this line used to say "1 minutes". */
    @Test
    fun `a window of one minute is said in the singular`() {
        assertEquals("1 minute", VaultChapterCopy.idleWindow(status(60_000), en))
        assertEquals("1 分钟", VaultChapterCopy.idleWindow(status(60_000), zh))
    }

    /**
     * A forty-second window said as "0 minutes" reads as "it does not re-lock",
     * which is the opposite of true — and it is the kind of wrong sentence a
     * reader would only discover by being surprised later.
     */
    @Test
    fun `a window shorter than a minute is never said as zero`() {
        assertEquals("under a minute", VaultChapterCopy.idleWindow(status(40_000), en))
    }

    @Test
    fun `a window that could not be read states nothing`() {
        assertNull(VaultChapterCopy.idleWindow(null, en))
        assertNull("a vault that never re-locks has no window to state", VaultChapterCopy.idleWindow(status(0), en))
    }

    @Test
    fun `each phase gets its own sentence, in both languages`() {
        val said = VaultPhase.entries.map { VaultChapterCopy.headline(it, en) }
        assertEquals("no two phases may share a sentence", said.size, said.toSet().size)
        for (phase in VaultPhase.entries) {
            assertNotEquals(
                VaultChapterCopy.headline(phase, en),
                VaultChapterCopy.headline(phase, zh),
            )
        }
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Pins the card's honest state trio, including the >128 KB refusal that no
// real sender can produce on the emulator (shell single-argument cap and
// Chrome's 100k-character share truncation); this test is that branch's
// regression guard.

package dev.typvia.share

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ShareCardStateTest {
    @Test
    fun missingTextRefusesWithTheNoTextMessage() {
        val state = shareCardState(null)
        assertEquals("Nothing to save — this share did not include text", state.metaLine)
        assertFalse(state.saveEnabled)
    }

    @Test
    fun blankTextRefusesWithTheHoldsNoTextMessage() {
        val state = shareCardState("  \n\t ")
        assertEquals("Nothing to save — the share holds no text", state.metaLine)
        assertFalse(state.saveEnabled)
    }

    @Test
    fun overLimitTextRefusesAndStillNamesTheCount() {
        val state = shareCardState("y".repeat(ShareInbox.TEXT_MAX_BYTES + 1))
        assertEquals("131073 characters · over the 128 KB limit, too long to save", state.metaLine)
        assertFalse(state.saveEnabled)
    }

    @Test
    fun multibyteTextIsMeasuredInUtf8BytesNotChars() {
        // 44,000 CJK chars are only 44k code points but 132k UTF-8 bytes —
        // over the limit even though the character count reads small.
        val state = shareCardState("测".repeat(44_000))
        assertEquals("44000 characters · over the 128 KB limit, too long to save", state.metaLine)
        assertFalse(state.saveEnabled)
    }

    @Test
    fun ordinaryTextCountsCodePointsAndOffersSave() {
        assertEquals(ShareCardState("1 character", true), shareCardState("好"))
        assertEquals(ShareCardState("2 characters", true), shareCardState("🎉!"))
    }

    @Test
    fun textAtExactlyTheLimitIsStillSavable() {
        val state = shareCardState("y".repeat(ShareInbox.TEXT_MAX_BYTES))
        assertTrue(state.saveEnabled)
        assertEquals("131072 characters", state.metaLine)
    }
}

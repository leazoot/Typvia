// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Joining an account: the rules that decide what the page may offer, what it
// may claim, and what it says when it refuses.

package dev.typvia.mobile

import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.UiLanguage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PairingTest {
    private val en = Translator(UiLanguage.En)

    /**
     * A code is read out loud to somebody holding another phone. Grouping is
     * what keeps it from being read wrong once and then blamed on the product.
     */
    @Test
    fun `a code is grouped for reading aloud`() {
        assertEquals("A1B2 C3D4 E5", PairingCode.grouped("A1B2C3D4E5"))
    }

    /**
     * Rounded up, never divided down: a code with forty seconds left still
     * works, and "0 minutes" tells the reader it is dead.
     */
    @Test
    fun `a code with under a minute left is not reported as zero minutes`() {
        assertEquals(1, PairingCode("A", 40).minutesLeft)
        assertFalse(PairingCode("A", 40).hasExpired)
    }

    /**
     * A WebDAV folder has no session server, so nobody promised a window. A
     * screen that counts one down anyway states an expiry no one undertook —
     * and by counting from zero, calls a working code dead the moment it
     * appears.
     */
    @Test
    fun `a code nobody promised a window for says nothing about validity`() {
        val noWindow = PairingCode("A1B2C3", null)

        assertNull(PairingCopy.validity(noWindow, en))
        assertFalse("a missing window is not an expiry", noWindow.hasExpired)

        val spent = PairingCode("A1B2C3", 0)
        assertTrue(PairingCopy.validity(spent, en)!!.contains("expired"))
    }

    /** The last minute of a code's life is "1 more minute", not "1 more minutes". */
    @Test
    fun `the last minute is said in the singular`() {
        assertEquals("Good for 1 more minute", PairingCopy.validity(PairingCode("A1B2C3", 40), en))
        assertEquals("Good for 4 more minutes", PairingCopy.validity(PairingCode("A1B2C3", 200), en))
    }

    /**
     * Only a definite no stops the reader. "Could not read" is not "cannot" —
     * a page that refuses on the strength of a question it never got an answer
     * to is refusing on its own guess.
     */
    @Test
    fun `an unknown key store lets the reader try, a definite no does not`() {
        val filled = PairingAsk(address = "https://example.invalid", accountId = "acct")

        assertTrue(filled.canBegin(canHoldAKey = null))
        assertTrue(filled.canBegin(canHoldAKey = true))
        assertFalse(filled.canBegin(canHoldAKey = false))
    }

    /**
     * What an address has to look like is the core's answer. This side only
     * knows that empty fields have nothing to send — and that a WebDAV folder
     * is not asked for an account id, because the account is read from the
     * storage itself.
     */
    @Test
    fun `empty fields have nothing to send, and webdav is not asked for an id`() {
        assertFalse(PairingAsk(accountId = "acct").canBegin(true))
        assertFalse(
            "the server form needs the id the other device shows",
            PairingAsk(address = "https://example.invalid").canBegin(true),
        )
        assertTrue(
            PairingAsk(
                target = PairingTarget.Webdav,
                address = "https://example.invalid/dav",
            ).canBegin(true),
        )
    }

    /**
     * The question a refused reader actually has is whether their library is
     * now half somebody else's. Every refusal answers it before it says
     * anything else about itself.
     */
    @Test
    fun `every refusal ends by saying nothing was installed`() {
        for (refusal in PairingRefusal.entries) {
            val said = PairingCopy.sentence(refusal, en)

            assertTrue(
                "$refusal does not say what is still true",
                said.endsWith(PairingCopy.nothingInstalled(en)),
            )
        }
    }

    /**
     * A dropped session is back at the first step, not further along: the
     * reader is being asked for a new code, and a page numbering that "2 / 2"
     * would be telling them they are nearly done.
     */
    @Test
    fun `a drop is back at the first step`() {
        assertEquals(1, PairingStep.Offering("A1B2C3").number)
        assertEquals(1, PairingStep.Dropped.number)
        assertEquals(2, PairingStep.Comparing("7F2K9Q", "ab-cd").number)
        assertEquals(2, PairingStep.Joined.number)
    }

    /**
     * Half a string is not something a reader can have compared, so the verb
     * that installs is unreachable until the whole of it has landed.
     */
    @Test
    fun `the reveal is only complete once every character is on screen`() {
        val half = SasReveal(SAS, 10)

        assertFalse(half.isComplete)
        assertEquals(0.5f, half.progress, 0.001f)
        assertTrue(SasReveal(SAS, 20).isComplete)
        // Asked for more than there are: a reveal cannot run past its own end.
        assertEquals(20, SasReveal(SAS, 99).shown)
        assertFalse("an empty string is not a completed comparison", SasReveal("", 0).isComplete)
    }

    /**
     * The separators are the core's way of grouping for reading aloud, not
     * something either reader compares. Counting them would make the reveal
     * beat on a hyphen and the line under the characters lag behind them.
     */
    @Test
    fun `the separators are not characters to compare`() {
        assertEquals(20, SasReveal(SAS, 0).characters.size)
        assertFalse("a separator is not one of the characters", SasReveal(SAS, 20).characters.contains('-'))
        assertEquals("ABCDE FGHIJ", SasReveal(SAS, 10).spoken)
        assertEquals("", SasReveal(SAS, 0).spoken)
    }

    /**
     * The one the walkthrough caught: the page was written for a six-character
     * check and the core hands over four groups of five, so a single line of
     * them ran off the edge of the phone and the reader was asked whether a
     * string they could only half see matched. Rows are what makes the check
     * makeable, and no row may carry more than one line's worth.
     */
    @Test
    fun `the whole string is laid out, in rows that fit`() {
        val rows = SasReveal(SAS, 20).rows

        assertEquals(20, rows.flatten().sumOf { it.characters.size })
        assertTrue(
            "a row wider than the narrowest phone is a check nobody can make",
            rows.all { row -> row.sumOf { it.characters.size } <= SasReveal.CHARACTERS_PER_ROW },
        )
        assertEquals(listOf("ABCDE", "FGHIJ"), rows.first().map { it.text })
        // Groups stay whole: half a group on each line is how a comparison is
        // read wrong.
        assertTrue(rows.all { row -> row.all { it.characters.size == 5 } })
    }

    /**
     * Characters that have not landed hold their place, so the string does not
     * crawl sideways as the rest of it arrives.
     */
    @Test
    fun `a group keeps its width while it is still arriving`() {
        val rows = SasReveal(SAS, 12).rows

        assertEquals(listOf("ABCDE", "FGHIJ"), rows[0].map { it.text })
        assertEquals(listOf("KL   ", "     "), rows[1].map { it.text })
    }

    private companion object {
        /** The shape the core actually hands over: four groups of five. */
        const val SAS = "ABCDE-FGHIJ-KLMNO-PQRST"
    }
}

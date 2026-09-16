// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.UiLanguage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.typvia_mobile_ffi.CoreException

/** What a snippet's page may ask of a model, and what it says about the answer. */
class AiStripTest {
    private val en = Translator(UiLanguage.En)
    private val zh = Translator(UiLanguage.Zh)

    /**
     * An action states where its text may come from, and the core refuses a
     * run from anywhere else. A page that offered one it cannot supply would
     * be offering a verb whose only outcome is a refusal.
     */
    @Test
    fun `only an action that takes a snippet's own body is offered here`() {
        assertEquals("snippet", AiKeeper.SOURCE)
        assertTrue(AiActionRow("a", "Shorten", "snippet").canRunHere)
        assertFalse(AiActionRow("b", "Fix the clipboard", "clipboard").canRunHere)
        assertFalse(AiActionRow("c", "Whatever is selected", "selection").canRunHere)
    }

    /**
     * One kind per kind the core has. The first run of this strip reported a
     * provider that rejected the request as "that action is no longer here",
     * because a rejection and a missing record had been folded together here.
     */
    @Test
    fun `each of the core's refusals keeps its own kind`() {
        assertEquals(AiRefusal.NotUsable, AiRefusal.of(CoreException.Validation("bad model")))
        assertEquals(AiRefusal.NoResult, AiRefusal.of(CoreException.Conflict("provider rejected")))
        assertEquals(AiRefusal.Unreachable, AiRefusal.of(CoreException.Unavailable("timeout")))
        assertEquals(
            AiRefusal.NotPermitted,
            AiRefusal.of(CoreException.PermissionDenied("vault shut")),
        )
        assertEquals(AiRefusal.Missing, AiRefusal.of(CoreException.NotFound()))
        assertEquals(AiRefusal.Storage, AiRefusal.of(IllegalStateException("disk on fire")))
    }

    /**
     * The kind of failure, never the engine's own words. An engine message is
     * written for whoever reads a log, and this product renders one language
     * at a time.
     */
    @Test
    fun `no sentence repeats what the engine said`() {
        val leaked = AiRefusal.of(CoreException.Conflict("egress blocked: AKIA_FAKE_SECRET"))
        assertEquals(AiRefusal.NoResult, leaked)
        for (kind in AiRefusal.entries) {
            for (sentence in listOf(AiStripCopy.refusal(kind, en), AiChapterCopy.refusal(kind, en))) {
                assertFalse(sentence.contains("reason="))
                assertFalse(sentence.contains("AKIA_FAKE_SECRET"))
            }
            assertNotEquals(AiStripCopy.refusal(kind, en), AiStripCopy.refusal(kind, zh))
        }
    }

    /**
     * The core folds the egress gate's refusal in with a missing key and a
     * provider saying no, so this layer cannot tell them apart. The sentence
     * for that kind therefore claims nothing about what left the device —
     * claiming either way would be a guess printed as a fact.
     */
    @Test
    fun `the sentence for a folded refusal claims nothing about what left`() {
        val sentence = AiStripCopy.refusal(AiRefusal.NoResult, en)
        assertFalse(sentence.contains("left"))
        assertFalse(sentence.contains("sent"))
        assertTrue(sentence.contains("nothing here changed"))
    }

    /**
     * A change that did not take leaves the engine exactly as it was. The
     * chapter must never read as "off" over something still configured.
     */
    @Test
    fun `the chapter never implies a failed change removed the engine`() {
        for (kind in AiRefusal.entries) {
            if (kind == AiRefusal.Missing) continue
            val sentence = AiChapterCopy.refusal(kind, en)
            assertTrue(
                kind.name,
                sentence.contains("as it was") || sentence.contains("the engine can use"),
            )
        }
    }

    @Test
    fun `what was hidden on the way out is named, and silence means nothing was`() {
        assertNull(AiStripCopy.masked(emptyList(), en))
        assertEquals(
            "Masked before it left: api_key, password",
            AiStripCopy.masked(listOf("api_key", "password"), en),
        )
        assertEquals("出门前被遮掉的:api_key", AiStripCopy.masked(listOf("api_key"), zh))
    }

    /**
     * What came back is the expensive part. A save that fails must not take it
     * off the screen — that would turn one failure into two.
     */
    @Test
    fun `a failed save keeps the words it failed to save`() {
        val produced = AiStripPhase.Produced("shorter text", emptyList())
        val afterFailure = produced.copy(refusal = AiRefusal.Storage)
        assertEquals("shorter text", afterFailure.text)
        assertEquals(AiRefusal.Storage, afterFailure.refusal)
        assertNull(produced.refusal)
    }
}

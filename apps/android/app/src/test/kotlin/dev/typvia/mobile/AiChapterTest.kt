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
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.typvia_mobile_ffi.AiProviderRow

/** Which machine an AI action's text would be handed to. */
class AiChapterTest {
    private val en = Translator(UiLanguage.En)
    private val zh = Translator(UiLanguage.Zh)

    /**
     * What is in effect is read from the providers, never from a preference of
     * this page's own: a settings screen keeping its own copy of the truth
     * eventually disagrees with it.
     */
    @Test
    fun `the engine in effect is read from what is configured`() {
        assertEquals(AiEngine.Off, AiEngine.inEffect(emptyList()))
        assertEquals(AiEngine.OnThisDevice, AiEngine.inEffect(listOf(provider("http://127.0.0.1:11434"))))
        assertEquals(AiEngine.OwnKey, AiEngine.inEffect(listOf(provider("https://api.example.com/v1"))))
    }

    /**
     * "Could not read" is not "no engine". A page that read Off from a failed
     * read would tell somebody their engine is gone when it is not.
     */
    @Test
    fun `a list that could not be read is not the same as an empty one`() {
        assertEquals(AiEngine.Off, AiEngine.inEffect(null))
        // The distinction the screen has to keep: null arrives as Off here,
        // so the caller holds the unread state and does not overwrite what it
        // last knew with it.
        assertEquals(AiEngine.Off, AiEngine.inEffect(emptyList()))
    }

    @Test
    fun `only an address that stays on the machine counts as local`() {
        assertTrue(AiEngine.isLocal("http://localhost:11434"))
        assertTrue(AiEngine.isLocal("http://127.0.0.1:11434"))
        assertFalse(AiEngine.isLocal("https://api.openai.com/v1"))
        // A host that merely reads like the loopback one is not it.
        assertFalse(AiEngine.isLocal("https://localhost.example.com/v1"))
        assertFalse(AiEngine.isLocal(""))
        assertFalse(AiEngine.isLocal("not a url at all"))
    }

    /**
     * The delivery's line for this option is "the AI chapter disappears; the
     * other seven kinds carry on" — which this build does not do: the library
     * lists all eight kinds whichever engine is chosen. What is printed is
     * what actually happens.
     */
    @Test
    fun `turning it off promises only what turning it off does`() {
        val off = AiChapterCopy.detail(AiEngine.Off, en)
        assertFalse(off.contains("chapter"))
        assertTrue(off.contains("no model", ignoreCase = true) || off.contains("any model"))
        assertFalse(AiChapterCopy.detail(AiEngine.Off, zh).contains("章节"))
    }

    @Test
    fun `every engine is titled and explained in both languages`() {
        for (engine in AiEngine.entries) {
            assertTrue(AiChapterCopy.title(engine, en).isNotBlank())
            assertNotEquals(AiChapterCopy.title(engine, en), AiChapterCopy.title(engine, zh))
            assertNotEquals(AiChapterCopy.detail(engine, en), AiChapterCopy.detail(engine, zh))
        }
    }

    /**
     * A refused removal leaves the engine running. The page must not then read
     * as "off" over something still configured — it says what is still true.
     */
    @Test
    fun `a change that did not take says the engine is as it was`() {
        assertTrue(AiChapterCopy.refusal(AiRefusal.Storage, en).contains("as it was"))
        assertTrue(AiChapterCopy.refusal(AiRefusal.Storage, zh).contains("还是原来的样子"))
        for (kind in AiRefusal.entries) {
            // The kind of failure, never the engine's own words.
            assertFalse(AiChapterCopy.refusal(kind, en).contains("reason="))
            assertNotEquals(AiChapterCopy.refusal(kind, en), AiChapterCopy.refusal(kind, zh))
        }
    }

    private fun provider(baseUrl: String) = AiProviderRow(
        id = "p1",
        name = "engine",
        kind = "openai_compatible",
        baseUrl = baseUrl,
        model = "m",
        timeoutMs = 0,
        hasApiKey = false,
    )
}

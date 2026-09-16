// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.UiLanguage
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** What the keyboard chapter may state about a keyboard it does not run. */
class KeyboardChapterTest {
    private val en = Translator(UiLanguage.En)
    private val zh = Translator(UiLanguage.Zh)

    /**
     * "Could not ask" is not "off". A page that printed "not on yet" over an
     * unanswered question would send the reader to a settings screen where
     * they find it was on the whole time.
     */
    @Test
    fun `an unanswered question is not answered as off`() {
        val unknown = KeyboardChapterCopy.headline(null, en)
        val off = KeyboardChapterCopy.headline(false, en)
        val on = KeyboardChapterCopy.headline(true, en)

        assertNotEquals(unknown, off)
        assertNotEquals(unknown, on)
        assertFalse(unknown.contains("not switched on"))
        assertTrue(off.contains("not switched on"))
    }

    @Test
    fun `every state is said in both languages and never in one sentence`() {
        for (state in listOf(true, false, null)) {
            val english = KeyboardChapterCopy.headline(state, en)
            val chinese = KeyboardChapterCopy.headline(state, zh)
            assertTrue(english.isNotBlank())
            assertNotEquals(english, chinese)
            // One language at a time: a sentence carrying both halves is the
            // mixed surface the delivery forbids.
            assertFalse(chinese.contains("switched"))
        }
        assertNotEquals(
            KeyboardChapterCopy.openSystemList(en),
            KeyboardChapterCopy.openSystemList(zh),
        )
        assertNotEquals(
            KeyboardChapterCopy.switchToIt(en),
            KeyboardChapterCopy.switchToIt(zh),
        )
    }

    /**
     * The product's own name is a name, not a word to translate: a reader
     * hunting for it in the system's chooser is looking for "Typvia".
     */
    @Test
    fun `the chooser verb names the keyboard the reader is looking for`() {
        for (tr in listOf(en, zh)) {
            assertTrue(KeyboardChapterCopy.switchToIt(tr).contains("Typvia"))
        }
    }
}

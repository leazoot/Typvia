// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.ime

import org.junit.Assert.assertEquals
import org.junit.Test

class TypeMarkTest {
    @Test
    fun mapsCoreTypesToTwoLetterMarks() {
        assertEquals("TX", TypeMark.mark("text"))
        assertEquals("TX", TypeMark.mark("markdown"))
        assertEquals("CD", TypeMark.mark("code"))
        assertEquals("CM", TypeMark.mark("command"))
        assertEquals("PR", TypeMark.mark("prompt"))
        assertEquals("TP", TypeMark.mark("template"))
        assertEquals("SC", TypeMark.mark("sensitive"))
        assertEquals("AI", TypeMark.mark("ai_action"))
        assertEquals("LK", TypeMark.mark("link"))
    }

    @Test
    fun unknownTypesDegradeToPlainText() {
        // Forward compatibility: a newer snapshot may carry types this build
        // does not know; they must present as plain text, never crash.
        assertEquals("TX", TypeMark.mark("hologram"))
        assertEquals("TX", TypeMark.mark(""))
    }

    @Test
    fun screenReadersGetFullWords() {
        assertEquals("text", TypeMark.word("TX"))
        assertEquals("secret", TypeMark.word("SC"))
        assertEquals("AI action", TypeMark.word("AI"))
        assertEquals("text", TypeMark.word("??"))
    }
}

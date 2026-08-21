// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Host-JVM coverage of the password red line: every password variation
// refuses, and the class-bit check keeps overlapping variation values (URI
// vs number-password) from misfiring.

package dev.typvia.ime

import android.text.InputType
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PasswordFieldTest {
    @Test
    fun detectsTextPasswordVariants() {
        assertTrue(
            PasswordField.isPassword(
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD,
            ),
        )
        assertTrue(
            PasswordField.isPassword(
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
            ),
        )
        assertTrue(
            PasswordField.isPassword(
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
            ),
        )
    }

    @Test
    fun detectsNumberPassword() {
        assertTrue(
            PasswordField.isPassword(
                InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_VARIATION_PASSWORD,
            ),
        )
    }

    @Test
    fun ignoresPlainTextEmailAndMultiline() {
        assertFalse(PasswordField.isPassword(InputType.TYPE_CLASS_TEXT))
        assertFalse(
            PasswordField.isPassword(
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS,
            ),
        )
        assertFalse(
            PasswordField.isPassword(
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE,
            ),
        )
        assertFalse(PasswordField.isPassword(InputType.TYPE_CLASS_NUMBER))
    }

    @Test
    fun uriVariationSharingNumberPasswordBitsIsNotAPassword() {
        // TYPE_TEXT_VARIATION_URI == TYPE_NUMBER_VARIATION_PASSWORD (0x10):
        // without the class check a URI field would be refused.
        assertFalse(
            PasswordField.isPassword(
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI,
            ),
        )
    }

    @Test
    fun datetimeClassNeverReadsAsPassword() {
        assertFalse(PasswordField.isPassword(InputType.TYPE_CLASS_DATETIME))
    }
}

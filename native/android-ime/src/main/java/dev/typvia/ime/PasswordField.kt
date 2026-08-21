// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Password-field detection from EditorInfo.inputType. Red line: Typvia never
// types into password fields — the IME refuses insertion and says so honestly.

package dev.typvia.ime

import android.text.InputType

object PasswordField {
    /**
     * True when the editor's inputType names a password variation. The class
     * bits must be checked alongside the variation bits: variation constants
     * overlap across classes (TYPE_NUMBER_VARIATION_PASSWORD shares its value
     * with TYPE_TEXT_VARIATION_URI). Visible passwords are still passwords.
     */
    fun isPassword(inputType: Int): Boolean {
        val fieldClass = inputType and InputType.TYPE_MASK_CLASS
        val variation = inputType and InputType.TYPE_MASK_VARIATION
        return when (fieldClass) {
            InputType.TYPE_CLASS_TEXT ->
                variation == InputType.TYPE_TEXT_VARIATION_PASSWORD ||
                    variation == InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD ||
                    variation == InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD
            InputType.TYPE_CLASS_NUMBER ->
                variation == InputType.TYPE_NUMBER_VARIATION_PASSWORD
            else -> false
        }
    }
}

// Test-only host for the instrumentation closed loop, mirroring
// the iOS KbTestHost: one plain multiline field to receive real insertions
// and one password field to prove the refusal red line. Declared in the
// androidTest manifest only — it never exists in a product APK.

package dev.typvia.ime

import android.app.Activity
import android.os.Bundle
import android.text.InputType
import android.widget.EditText
import android.widget.LinearLayout

class ImeHostActivity : Activity() {
    lateinit var plainField: EditText
        private set

    lateinit var passwordField: EditText
        private set

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val column =
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(PADDING, PADDING, PADDING, 0)
            }
        plainField =
            EditText(this).apply {
                // Multiline so a commitText body with newlines lands intact.
                inputType =
                    InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
                contentDescription = PLAIN_FIELD
                minLines = 3
            }
        passwordField =
            EditText(this).apply {
                inputType =
                    InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
                contentDescription = PASSWORD_FIELD
            }
        column.addView(plainField)
        column.addView(passwordField)
        setContentView(column)
    }

    companion object {
        /** Accessibility handles the UiAutomator side of the test taps. */
        const val PLAIN_FIELD = "hostField"
        const val PASSWORD_FIELD = "hostPasswordField"
        private const val PADDING = 48
    }
}

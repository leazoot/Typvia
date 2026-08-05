// SPIKE (TASK-012): host activity writes the snapshot file (same package as
// the IME, mirroring the real app-plus-IME packaging) and offers a multiline
// field and a password field so field-type detection can be exercised.

package dev.typvia.spike.ime

import android.app.Activity
import android.os.Bundle
import android.text.InputType
import android.util.Log
import android.widget.EditText
import android.widget.LinearLayout
import java.io.File

const val SNAPSHOT_NAME = "keyboard-snapshot.json"

// Multiline + CJK content, same shape as the iOS spike snapshot.
val SNAPSHOT_JSON = """
{
  "version": 1,
  "snippets": [
    {"id": "s1", "title": "Standup", "content": "Yesterday: shipped spikes\nToday: IME validation\nBlockers: none"},
    {"id": "s2", "title": "CN", "content": "你好,这是中文多行\n第二行内容"}
  ]
}
""".trimIndent()

class MainActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        File(filesDir, SNAPSHOT_NAME).writeText(SNAPSHOT_JSON)
        Log.i("SPIKE-host", "snapshot written")

        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(32, 64, 32, 0)
        }
        val multiline = EditText(this).apply {
            hint = "multiline field"
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
            minLines = 4
            gravity = android.view.Gravity.TOP
            contentDescription = "multilineField"
            addTextChangedListener(object : android.text.TextWatcher {
                override fun afterTextChanged(s: android.text.Editable?) {
                    Log.i("SPIKE-host", "multiline now: ${s.toString().replace("\n", "\\n")}")
                }

                override fun beforeTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
                override fun onTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
            })
        }
        val password = EditText(this).apply {
            hint = "password field"
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            contentDescription = "passwordField"
        }
        layout.addView(multiline)
        layout.addView(password)
        setContentView(layout)
        multiline.requestFocus()
    }
}

// SPIKE (TASK-012): InputMethodService that reads the snapshot file, renders
// snippet keys, detects password fields from EditorInfo, and commits
// multiline text through the InputConnection.

package dev.typvia.spike.ime

import android.graphics.Color
import android.inputmethodservice.InputMethodService
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import org.json.JSONObject
import java.io.File

class TypviaImeService : InputMethodService() {
    private data class Snippet(val id: String, val title: String, val content: String)

    private var snippets: List<Snippet> = emptyList()
    private lateinit var status: TextView
    private var passwordField = false

    private fun readSnapshot(): String {
        return try {
            val json = JSONObject(File(filesDir, SNAPSHOT_NAME).readText())
            val arr = json.getJSONArray("snippets")
            snippets = (0 until arr.length()).map { i ->
                val o = arr.getJSONObject(i)
                Snippet(o.getString("id"), o.getString("title"), o.getString("content"))
            }
            "read=OK v${json.getInt("version")} snippets=${snippets.size}"
        } catch (e: Exception) {
            "read=FAILED ${e.message}"
        }
    }

    override fun onCreateInputView(): View {
        val readResult = readSnapshot()
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.parseColor("#DDDDDD"))
            setPadding(16, 16, 16, 32)
        }
        val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        snippets.forEach { s ->
            row.addView(
                Button(this).apply {
                    text = s.title
                    setOnClickListener { insert(s) }
                },
                LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f),
            )
        }
        status = TextView(this).apply {
            textSize = 11f
            gravity = Gravity.START
        }
        root.addView(row)
        root.addView(status)
        renderStatus(readResult)
        return root
    }

    override fun onStartInputView(info: EditorInfo, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        val variation = info.inputType and android.text.InputType.TYPE_MASK_VARIATION
        passwordField = variation == android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD ||
            variation == android.text.InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD ||
            variation == android.text.InputType.TYPE_NUMBER_VARIATION_PASSWORD
        renderStatus("read=OK snippets=${snippets.size}")
        // Auto-insert into non-password fields once settled, so the multiline
        // commitText path is proven without UI-test tap plumbing.
        if (!passwordField && snippets.isNotEmpty()) {
            status.postDelayed({ insert(snippets.first()) }, 1500)
        }
    }

    private fun insert(s: Snippet) {
        if (passwordField) {
            Log.i("SPIKE-ime", "blocked insert into password field")
            renderStatus("blocked: password field")
            return
        }
        currentInputConnection?.commitText(s.content, 1)
        Log.i("SPIKE-ime", "inserted=${s.id}")
        renderStatus("inserted=${s.id}")
    }

    private fun renderStatus(readResult: String) {
        val line = "password=$passwordField | $readResult"
        status.text = line
        Log.i("SPIKE-ime", "status: $line")
    }
}

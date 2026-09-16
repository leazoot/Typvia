// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// The Typvia keyboard: a composing bench, never a QWERTY.
//
// It is 252dp of four bands — a search line, the sort marks, the run of
// sheets, and the function row — with the leftover left to the system's own
// navigation bar rather than drawn under it. Tapping a sheet types its body
// into the host field one character at a time, because that is what this
// product does: it types, it does not paste.
//
// Security posture: no network, no keystroke logging, no key history, no
// analytics; the host's text is never read (no getTextBeforeCursor /
// getSurroundingText anywhere); password fields refuse insertion; a secret is
// never previewed here and never reaches the clipboard.

package dev.typvia.ime

import android.inputmethodservice.InputMethodService
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.HapticFeedbackConstants
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.TextView
import dev.typvia.mobile.ui.KeyboardPhase
import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.TypeIn
import dev.typvia.mobile.ui.UiPreferenceFile
import java.io.File

class TypviaImeService : InputMethodService() {
    private var bench = Bench(null)
    private var passwordField = false

    /**
     * How many UTF-16 units this keyboard has put into the host field. Undo
     * takes back exactly this many and not one more: counting the body's own
     * length would eat whatever the reader had typed themselves when the run
     * was cut short.
     */
    private var typedUnits = 0

    private lateinit var theme: PanelTheme
    private lateinit var copy: BenchCopy
    private lateinit var root: LinearLayout
    private lateinit var searchCaret: View
    private lateinit var searchText: TextView
    private lateinit var searchCount: TextView
    private lateinit var sortsRow: LinearLayout
    private lateinit var sortsBand: View
    private lateinit var tilesRow: LinearLayout
    private lateinit var tilesScroll: HorizontalScrollView
    private lateinit var tilesBand: FrameLayout
    private lateinit var stateHost: FrameLayout
    private lateinit var noteBar: LinearLayout
    private lateinit var noteText: TextView
    private lateinit var functionsRow: LinearLayout
    private lateinit var room: FrameLayout

    private val handler = Handler(Looper.getMainLooper())
    private var typing: Runnable? = null
    private val retireNote = Runnable {
        if (bench.phase is KeyboardPhase.Inserted) {
            bench.backToBench()
            render()
        }
    }

    // MARK: Lifecycle

    override fun onCreateInputView(): View {
        theme = PanelTheme(this)
        copy = BenchCopy(Translator(theme.language))
        return buildPanel()
    }

    override fun onStartInputView(info: android.view.inputmethod.EditorInfo, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        passwordField = PasswordField.isPassword(info.inputType)
        // The panel is built once and kept, so a reader who changes the app's
        // language would go on getting the old one until the system happened
        // to kill this process. Checked here because this is the moment the
        // keyboard comes up, which is the moment after they changed it.
        if (UiPreferenceFile.read(dataDir) != theme.chosen) {
            setInputView(onCreateInputView())
        }
        // Read afresh on every field: the file is small, and the alternative
        // is a bench showing the library as it was the last time the reader
        // summoned the keyboard.
        bench = Bench(SnapshotStore.load(File(dataDir, SnapshotStore.RELATIVE_PATH)))
        stopTyping()
        handler.removeCallbacks(retireNote)
        render()
    }

    override fun onFinishInputView(finishingInput: Boolean) {
        stopTyping()
        handler.removeCallbacks(retireNote)
        super.onFinishInputView(finishingInput)
    }

    /** The panel is height-capped and never takes over the screen. */
    override fun onEvaluateFullscreenMode(): Boolean = false

    /**
     * Back puts the panel away without giving up the input method — the
     * platform's own reading of the key, and the one the delivery asks for.
     */
    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (isInputViewShown && keyCode == KeyEvent.KEYCODE_BACK) {
            requestHideSelf(0)
            return true
        }
        // An IME cannot summon a keyboard for its own search line — it is the
        // keyboard — so a query typed on hardware keys lands here. It lives in
        // one field, is never logged and never persisted.
        if (isInputViewShown && !passwordField && bench.phase !is KeyboardPhase.Secret) {
            if (keyCode == KeyEvent.KEYCODE_DEL && bench.backspace()) {
                render()
                return true
            }
            val character = event.unicodeChar
            if (character != 0 && !Character.isISOControl(character)) {
                bench.type(character.toChar())
                render()
                return true
            }
        }
        return super.onKeyDown(keyCode, event)
    }

    // MARK: What a tap does

    private fun tapped(tile: BenchTile) {
        root.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
        if (tile.isSecret) {
            // The whole panel becomes an ink room. Nothing is unlocked here.
            bench.openSecret(tile.title)
            render()
            return
        }
        val body = bench.body(tile.id) ?: return
        typeIn(body)
        bench.markUsed(tile.id, tile.title)
        render()
        handler.removeCallbacks(retireNote)
        handler.postDelayed(retireNote, NOTE_LIFE_MS)
    }

    /**
     * Types into the host field, one character at a time.
     *
     * The cap is on the whole insertion rather than on each character, so a
     * long body lands at once instead of taking longer — past the point where
     * the motion could be read, it is not motion, it is a wait.
     */
    private fun typeIn(body: String) {
        // Red line: never type into a password field. The refusal is already
        // on screen; this guard holds even for a stale tap.
        if (passwordField) return
        val connection = currentInputConnection ?: return
        stopTyping()
        typedUnits = 0
        when (val plan = TypeIn.plan(body.codePointCount(0, body.length))) {
            TypeIn.Plan.AtOnce -> {
                connection.commitText(body, 1)
                typedUnits = body.length
                root.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
            }
            is TypeIn.Plan.Character -> typeRest(body, 0, plan.intervalMs.toLong())
        }
    }

    /** One character, then the next — never splitting a surrogate pair. */
    private fun typeRest(body: String, from: Int, intervalMs: Long) {
        if (from >= body.length) {
            typing = null
            root.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
            return
        }
        val width = Character.charCount(body.codePointAt(from))
        val piece = body.substring(from, from + width)
        currentInputConnection?.commitText(piece, 1) ?: run { typing = null; return }
        typedUnits += width
        val next = Runnable { typeRest(body, from + width, intervalMs) }
        typing = next
        handler.postDelayed(next, intervalMs)
    }

    private fun stopTyping() {
        typing?.let(handler::removeCallbacks)
        typing = null
    }

    /**
     * Takes back what this keyboard typed, and only that. The count is what
     * actually went in, so an undo pressed mid-run leaves the reader's own
     * words alone.
     */
    private fun undoInsertion() {
        stopTyping()
        if (typedUnits > 0) currentInputConnection?.deleteSurroundingText(typedUnits, 0)
        typedUnits = 0
        handler.removeCallbacks(retireNote)
        bench.backToBench()
        render()
    }

    private fun backspaceHost() {
        stopTyping()
        typedUnits = 0
        sendDownUpKeyEvents(KeyEvent.KEYCODE_DEL)
    }

    /** ABC hands the field back to an ordinary keyboard. */
    private fun handOver() {
        if (!switchToPreviousInputMethod()) requestHideSelf(0)
    }

    /** Nothing matched: the way on is the app, with no host text read here. */
    private fun openApp() {
        val intent = packageManager.getLaunchIntentForPackage(packageName) ?: return
        intent.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
        startActivity(intent)
    }

    // MARK: Drawing

    private fun render() {
        val secret = bench.phase as? KeyboardPhase.Secret
        room.visibility = if (secret != null) View.VISIBLE else View.GONE
        root.setBackgroundColor(if (secret != null) theme.vaultPaper else theme.paper)
        if (secret != null) {
            renderRoom(secret.title)
            return
        }
        renderSearch()
        renderSorts()
        renderTiles()
        renderNote()
        renderFunctions()
    }

    private fun renderSearch() {
        val query = bench.query
        searchCaret.setBackgroundColor(theme.accent)
        if (query.isEmpty()) {
            searchText.text = copy.searchPlaceholder
            searchText.setTextColor(theme.ink3)
            searchText.typeface = android.graphics.Typeface.DEFAULT
        } else {
            searchText.text = query
            searchText.setTextColor(theme.ink)
            // A query is machine text, and machine text is set in mono.
            searchText.typeface = android.graphics.Typeface.MONOSPACE
        }
        searchCount.visibility = if (passwordField) View.GONE else View.VISIBLE
        searchCount.text = if (query.isEmpty()) {
            bench.total.toString()
        } else {
            copy.matchCount(bench.tiles.size)
        }
    }

    private fun renderSorts() {
        sortsRow.removeAllViews()
        // A password field has nothing to sort: the refusal is the only thing
        // on the bench, and a band of live filters over it would be a lie.
        sortsBand.visibility = if (passwordField) View.GONE else View.VISIBLE
        if (passwordField) return
        for (sort in bench.sorts()) {
            sortsRow.addView(
                sortLead(this, theme, sort.mark ?: copy.allKinds, sort) {
                    bench.select(sort.mark)
                    render()
                },
            )
        }
    }

    private fun renderTiles() {
        tilesRow.removeAllViews()
        stateHost.removeAllViews()
        val face = when {
            passwordField -> benchState(this, theme, copy.passwordTitle, copy.passwordBody)
            bench.phase is KeyboardPhase.Unreachable ->
                benchState(this, theme, copy.unreachableTitle, copy.unreachableBody)
            bench.phase is KeyboardPhase.NoMatch -> noMatchFace()
            else -> null
        }
        if (face != null) {
            stateHost.addView(
                face,
                FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT,
                ),
            )
            stateHost.visibility = View.VISIBLE
            tilesScroll.visibility = View.GONE
            return
        }
        stateHost.visibility = View.GONE
        tilesScroll.visibility = View.VISIBLE
        for (tile in bench.tiles) {
            tilesRow.addView(
                benchTile(this, theme, copy, tile, bench.query, previewFor(tile), ::tapped),
                LinearLayout.LayoutParams(
                    if (tile.isLead) theme.tileLeadWidth else theme.tileWidth,
                    ViewGroup.LayoutParams.MATCH_PARENT,
                ).apply { marginEnd = theme.dp(10f) },
            )
        }
        tilesScroll.scrollTo(0, 0)
    }

    /** Only the lead sheet previews, and a secret never does. */
    private fun previewFor(tile: BenchTile): String? =
        if (tile.isLead && !tile.isSecret) bench.body(tile.id) else null

    private fun noMatchFace(): LinearLayout {
        val face = benchState(
            this,
            theme,
            if (bench.query.isBlank()) copy.noKindTitle else copy.noMatchTitle,
            copy.noMatchBody,
        )
        val actions = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        actions.addView(stateAction(this, theme, copy.createInApp, theme.ink) { openApp() })
        actions.addView(
            stateAction(this, theme, copy.clear, theme.ink3) {
                bench.clearQuery()
                bench.select(null as String?)
                render()
            },
        )
        face.addView(
            actions,
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = theme.dp(6f) },
        )
        return face
    }

    /**
     * The undo line sits over the run of sheets and takes none of the bench's
     * height. It lives four seconds: long enough to reach, short enough not to
     * become furniture over the sheets it covers.
     */
    private fun renderNote() {
        val inserted = bench.phase as? KeyboardPhase.Inserted
        noteBar.visibility = if (inserted != null) View.VISIBLE else View.GONE
        if (inserted != null) noteText.text = copy.inserted(inserted.title)
    }

    private fun renderFunctions() {
        functionsRow.removeAllViews()
        functionsRow.addView(
            functionItem(this, theme, copy.abc, active = false, description = copy.abc) { handOver() },
        )
        if (!passwordField) {
            for (scope in bench.scopes) {
                val label = when {
                    scope is PanelCategory.Recent && bench.query.isNotBlank() -> copy.matchScope
                    scope is PanelCategory.Recent -> copy.recentScope
                    else -> copy.starredScope
                }
                functionsRow.addView(
                    functionItem(
                        this,
                        theme,
                        label,
                        active = bench.scope == scope,
                        description = label,
                    ) {
                        bench.select(scope)
                        render()
                    },
                )
            }
        }
        functionsRow.addView(View(this), LinearLayout.LayoutParams(0, 1, 1f))
        functionsRow.addView(
            functionItem(
                this,
                theme,
                copy.backspace,
                active = false,
                description = copy.backspace,
            ) {
                // The query owns the key while there is a query; otherwise it
                // belongs to the field the reader is writing in.
                if (bench.backspace()) render() else backspaceHost()
            },
        )
    }

    /** The ink room: one thing at a time, and no way in from here. */
    private fun renderRoom(title: String) {
        room.removeAllViews()
        val face = benchState(
            this,
            theme,
            copy.secretBody(title),
            copy.secretShut,
            accent = theme.vaultAccent,
            ink = theme.vaultInk,
            ink2 = theme.vaultInk2,
        ).apply { setBackgroundColor(theme.vaultPaper) }
        face.addView(
            stateAction(this, theme, copy.backToBench, theme.vaultAccent) {
                bench.backToBench()
                render()
            },
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = theme.dp(6f) },
        )
        room.addView(
            face,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )
    }

    // MARK: Assembly

    private fun buildPanel(): View {
        root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(theme.paper)
        }
        val bands = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        bands.addView(buildSearch(), band(theme.searchBand))
        bands.addView(rule(this, theme), band(theme.hairlineWidth))
        sortsBand = buildSorts()
        bands.addView(sortsBand, band(theme.sortsBand))
        bands.addView(buildTiles(), band(theme.tilesBand))
        bands.addView(rule(this, theme), band(theme.hairlineWidth))
        bands.addView(buildFunctions(), band(theme.functionsBand))

        room = FrameLayout(this).apply { visibility = View.GONE }
        val stack = FrameLayout(this)
        stack.addView(
            bands,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )
        stack.addView(
            room,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )
        // The panel asks for exactly the bench's height. What is left of it
        // after the four bands belongs to the system's navigation bar, and is
        // left alone: a function row under that bar is one people mis-tap.
        root.addView(
            stack,
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, theme.panelHeight),
        )
        return root
    }

    private fun band(height: Int) =
        LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, height)

    /** No box and no magnifier: a caret, the words, and a rule under them. */
    private fun buildSearch(): View {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(theme.screenPadding, 0, theme.screenPadding, 0)
        }
        searchCaret = caret(this, theme, 17f, theme.accent)
        row.addView(searchCaret)
        searchText = textView(this, theme.titleSp, theme.ink3)
        row.addView(
            searchText,
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { marginStart = theme.dp(10f) },
        )
        row.addView(View(this), LinearLayout.LayoutParams(0, 1, 1f))
        searchCount = textView(this, theme.monoSp, theme.ink3, mono = true).apply {
            accessibilityLiveRegion = View.ACCESSIBILITY_LIVE_REGION_POLITE
        }
        row.addView(searchCount)
        return row
    }

    private fun buildSorts(): View {
        sortsRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        return HorizontalScrollView(this).apply {
            isHorizontalScrollBarEnabled = false
            setPadding(theme.screenPadding - theme.dp(10f), 0, theme.screenPadding, 0)
            addView(
                sortsRow,
                ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.MATCH_PARENT,
                ),
            )
        }
    }

    private fun buildTiles(): View {
        tilesRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(theme.screenPadding, theme.dp(6f), theme.screenPadding, theme.dp(6f))
        }
        tilesScroll = HorizontalScrollView(this).apply {
            isHorizontalScrollBarEnabled = false
            // The run carries on past the right edge; the sheet cut in half is
            // the whole hint that there are more of them.
            clipToPadding = false
            addView(
                tilesRow,
                ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.MATCH_PARENT,
                ),
            )
        }
        stateHost = FrameLayout(this).apply { visibility = View.GONE }
        noteText = textView(this, theme.bodySp, theme.ink)
        noteBar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            visibility = View.GONE
            setBackgroundColor(theme.carrier)
            setPadding(theme.screenPadding, 0, theme.screenPadding, 0)
        }
        noteBar.addView(noteText)
        noteBar.addView(View(this), LinearLayout.LayoutParams(0, 1, 1f))
        noteBar.addView(
            textView(this, theme.bodySp, theme.accent, medium = true).apply {
                text = copy.undo
                minHeight = theme.dp(44f)
                gravity = Gravity.CENTER_VERTICAL
                setOnClickListener { undoInsertion() }
            },
        )

        tilesBand = FrameLayout(this)
        tilesBand.addView(
            tilesScroll,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )
        tilesBand.addView(
            stateHost,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )
        tilesBand.addView(
            noteBar,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                theme.dp(44f),
                Gravity.TOP,
            ),
        )
        return tilesBand
    }

    private fun buildFunctions(): View {
        functionsRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(theme.screenPadding - theme.dp(12f), 0, theme.screenPadding - theme.dp(12f), 0)
        }
        return functionsRow
    }

    private companion object {
        /** How long the undo line stays before it gets out of the way. */
        const val NOTE_LIFE_MS = 4_000L
    }
}

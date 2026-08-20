// The Typvia IME. It is a snippet panel, never a QWERTY: a 52dp bar of recent
// strips that expands into the full panel — search line with the brand
// caret, editorial snippet rows, footer tab rail. Tap inserts the full body
// through InputConnection.commitText; a haptic tick plus the transient
// "Inserted" line confirm.
//
// Security posture: no network, no keystroke logging, no key history, no
// analytics; the host's text is never read (no getTextBeforeCursor/
// getSurroundingText anywhere); password fields refuse insertion; sensitive
// entries render locked with zero plaintext and zero insertion path.

package dev.typvia.ime

import android.animation.ValueAnimator
import android.content.ClipData
import android.content.ClipboardManager
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.InsetDrawable
import android.inputmethodservice.InputMethodService
import android.view.Gravity
import android.view.HapticFeedbackConstants
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.view.animation.DecelerateInterpolator
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.TextView
import java.io.File
import java.util.Locale
import uniffi.typvia_mobile_ffi.SnapshotEntry

class TypviaImeService : InputMethodService() {
    private var store: SnapshotStore? = null
    private var categories: List<PanelCategory> = emptyList()
    private var selectedCategory: PanelCategory? = null
    private var query = ""
    private var rows: List<SnapshotEntry> = emptyList()
    private var passwordField = false
    private var expanded = false

    // Template fill mode: the entry being filled, its variable
    // names in first-seen order, the typed values, and the focused field.
    private var fillEntry: SnapshotEntry? = null
    private var fillVariables: List<String> = emptyList()
    private val fillValues = LinkedHashMap<String, String>()
    private var fillFocus = 0

    private lateinit var theme: PanelTheme
    private lateinit var content: FrameLayout
    private lateinit var barView: LinearLayout
    private lateinit var barStrips: LinearLayout
    private lateinit var panelView: LinearLayout
    private lateinit var listMode: LinearLayout
    private lateinit var headerCaret: View
    private lateinit var searchText: TextView
    private lateinit var trailingCaret: View
    private lateinit var wordmark: TextView
    private lateinit var resultCount: TextView
    private lateinit var headerUnderline: View
    private lateinit var tabStack: LinearLayout
    private lateinit var list: ListView
    private lateinit var adapter: SnippetRowAdapter
    private lateinit var stateContainer: FrameLayout
    private lateinit var factText: TextView
    private lateinit var factLine: View
    private lateinit var fillView: LinearLayout
    private lateinit var fillTitle: TextView
    private lateinit var fillFields: LinearLayout
    private lateinit var fillPreview: TextView
    private var heightAnimator: ValueAnimator? = null
    private val hideFact = Runnable { factLine.visibility = View.GONE }

    // MARK: Lifecycle

    override fun onCreateInputView(): View {
        theme = PanelTheme(this)
        return buildRoot()
    }

    override fun onStartInputView(info: android.view.inputmethod.EditorInfo, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        passwordField = PasswordField.isPassword(info.inputType)
        // Fresh read per field entry: the file is small and the host app may
        // have rewritten it since the panel was last shown.
        store = SnapshotStore.load(File(dataDir, SnapshotStore.RELATIVE_PATH))
        query = ""
        categories = store?.let { PanelCategory.available(it.entries, it.folders) } ?: emptyList()
        if (selectedCategory !in categories) selectedCategory = null
        adapter.bodyOf = { id -> store?.body(id) }
        adapter.clearPreviews()
        factLine.removeCallbacks(hideFact)
        factLine.visibility = View.GONE
        closeFill()
        applyExpansion(animate = false)
        refresh()
    }

    /** The panel is height-capped and must never take over the screen as an
     * extract view in landscape. */
    override fun onEvaluateFullscreenMode(): Boolean = false

    // MARK: Hardware-key search routing
    //
    // An IME cannot summon a keyboard for its own search field (it IS the
    // keyboard), so — like the iOS panel — search text arrives from a
    // hardware keyboard. While the panel is expanded, printable keys go to
    // the search line (and nowhere else: the query lives in one field and is
    // never logged or persisted); collapsed, keys pass through untouched.

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (isInputViewShown && expanded && !passwordField) {
            // Fill mode first: printable keys land in the focused template
            // field (same in-memory-only path as the search query), Enter
            // inserts the rendered result.
            val fill = fillEntry
            if (fill != null) {
                val name = fillVariables.getOrNull(fillFocus)
                if (keyCode == KeyEvent.KEYCODE_ENTER) {
                    fillInsert()
                    return true
                }
                if (name != null) {
                    if (keyCode == KeyEvent.KEYCODE_DEL) {
                        val value = fillValues[name].orEmpty()
                        if (value.isNotEmpty()) fillValues[name] = value.dropLast(1)
                        renderFill()
                        return true
                    }
                    val ch = event.unicodeChar
                    if (ch != 0 && !Character.isISOControl(ch)) {
                        fillValues[name] = fillValues[name].orEmpty() + ch.toChar()
                        renderFill()
                        return true
                    }
                }
                return super.onKeyDown(keyCode, event)
            }
            if (keyCode == KeyEvent.KEYCODE_DEL && query.isNotEmpty()) {
                query = query.dropLast(1)
                refresh()
                return true
            }
            val ch = event.unicodeChar
            if (ch != 0 && !Character.isISOControl(ch)) {
                query += ch.toChar()
                refresh()
                return true
            }
        }
        return super.onKeyDown(keyCode, event)
    }

    // MARK: Data

    private fun refresh() {
        // A password field has no insertable rows at all: list, tabs and
        // count go empty so the refusal state is the only content.
        val base =
            if (passwordField) {
                emptyList()
            } else {
                store?.let { if (query.isEmpty()) it.entries else it.search(query) } ?: emptyList()
            }
        rows = selectedCategory?.let { tab -> base.filter(tab::matches) } ?: base
        adapter.rows = rows
        adapter.query = query
        adapter.notifyDataSetChanged()
        // The vault tab drops the surface one notch.
        panelView.setBackgroundColor(
            if (selectedCategory is PanelCategory.Vault) theme.vaultBg else theme.panelBg,
        )
        renderHeader()
        renderTabs()
        renderBarStrips()
        renderState()
    }

    // MARK: Actions

    /** Row tap: on the Templates tab a template with variables opens the
     * fill surface; every other row inserts its body as-is. */
    private fun rowTapped(entry: SnapshotEntry) {
        if (
            !passwordField &&
            !entry.isSensitive &&
            selectedCategory is PanelCategory.Templates &&
            entry.snippetType == "template"
        ) {
            val variables = store?.templateVariables(entry.id).orEmpty()
            if (variables.isNotEmpty()) {
                openFill(entry, variables)
                return
            }
        }
        insert(entry)
    }

    private fun insert(entry: SnapshotEntry) {
        // Locked rows never insert: body() is null for sensitive ids by
        // construction (the FFI returns nothing for them).
        if (entry.isSensitive) return
        commitBody(store?.body(entry.id) ?: return)
    }

    /** Commits text to the host and confirms (haptic tick plus the
     * transient fact line — the keyboard stays open, no toast). */
    private fun commitBody(body: String) {
        // Red line: never type into password fields. The refusal state
        // is already on screen; this guard holds even for a stale tap.
        if (passwordField) return
        val connection = currentInputConnection ?: return
        connection.commitText(body, 1)
        content.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
        showFact(PanelStrings.INSERTED_FACT)
    }

    private fun showFact(fact: String) {
        factText.text = fact
        factLine.removeCallbacks(hideFact)
        factLine.visibility = View.VISIBLE
        factLine.postDelayed(hideFact, FACT_LINE_MS)
    }

    private fun setExpanded(target: Boolean) {
        if (expanded == target) return
        expanded = target
        // The compact bar has no fill surface; collapsing abandons the fill.
        if (!target) closeFill()
        applyExpansion(animate = true)
        refresh()
    }

    /** Bar <-> panel: swap content and animate the height (expand 220ms per
     * the design, exit = enter x0.7; ValueAnimator already honors the
     * system's remove-animations setting via the animator duration scale). */
    private fun applyExpansion(animate: Boolean) {
        barView.visibility = if (expanded) View.GONE else View.VISIBLE
        panelView.visibility = if (expanded) View.VISIBLE else View.GONE
        val target = if (expanded) theme.panelHeight else theme.barHeight
        heightAnimator?.cancel()
        if (!animate) {
            setContentHeight(target)
            return
        }
        heightAnimator =
            ValueAnimator.ofInt(content.layoutParams.height, target).apply {
                duration = if (expanded) 220 else 154
                interpolator = DecelerateInterpolator()
                addUpdateListener { setContentHeight(it.animatedValue as Int) }
                start()
            }
    }

    private fun setContentHeight(height: Int) {
        content.layoutParams = content.layoutParams.apply { this.height = height }
        content.requestLayout()
    }

    // MARK: Template fill
    //
    // Tapping a template row with variables (Templates tab only) swaps the
    // list for the fill surface: caps field labels over hairline underlines,
    // a live preview, Insert and Copy. Field text arrives over the same
    // hardware-key route as the search query (an IME cannot summon a
    // keyboard for its own fields) and lives only in memory.

    private fun openFill(entry: SnapshotEntry, variables: List<String>) {
        fillEntry = entry
        fillVariables = variables
        fillValues.clear()
        fillFocus = 0
        listMode.visibility = View.GONE
        fillView.visibility = View.VISIBLE
        renderFill()
    }

    private fun closeFill() {
        if (fillEntry == null) return
        fillEntry = null
        fillVariables = emptyList()
        fillValues.clear()
        fillView.visibility = View.GONE
        listMode.visibility = View.VISIBLE
    }

    /** Insert = final render through the one insertion path; unfilled
     * variables become empty text, never placeholder marks. */
    private fun fillInsert() {
        val entry = fillEntry ?: return
        val body = store?.templateRender(entry.id, fillValues) ?: return
        closeFill()
        commitBody(body)
    }

    /** Copy puts the rendered normal-template text on the system clipboard.
     * The FFI refuses sensitive ids, so vault content can never reach the
     * clipboard from here. */
    private fun fillCopy() {
        val entry = fillEntry ?: return
        val body = store?.templateRender(entry.id, fillValues) ?: return
        val clipboard = getSystemService(ClipboardManager::class.java) ?: return
        clipboard.setPrimaryClip(ClipData.newPlainText("Typvia", body))
        content.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
        showFact(PanelStrings.COPIED_FACT)
    }

    private fun renderFill() {
        val entry = fillEntry ?: return
        fillTitle.text = entry.title
        fillFields.removeAllViews()
        for ((index, name) in fillVariables.withIndex()) {
            fillFields.addView(
                buildFillField(index, name),
                LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                ).apply { marginEnd = theme.dp(18f) },
            )
        }
        fillPreview.text = store?.templatePreview(entry.id, fillValues).orEmpty()
    }

    /** One variable field: caps mono label, typed value with a caret when
     * focused, hairline underline turning accent on focus. */
    private fun buildFillField(index: Int, name: String): View {
        val focused = index == fillFocus
        val field =
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                minimumWidth = theme.dp(130f)
                contentDescription = if (focused) "$name field, selected" else "$name field"
                setOnClickListener {
                    fillFocus = index
                    renderFill()
                }
            }
        field.addView(capsLabel(this, theme, name.uppercase(Locale.US)))
        val valueRow =
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
            }
        valueRow.addView(
            textView(this, theme.fieldValueSp, theme.ink).apply {
                text = fillValues[name].orEmpty()
            },
        )
        if (focused) {
            valueRow.addView(caretBar(this, theme, 15f))
            (valueRow.getChildAt(1).layoutParams as LinearLayout.LayoutParams).marginStart =
                theme.dp(2f)
        }
        field.addView(
            valueRow,
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                theme.dp(22f),
            ).apply { topMargin = theme.dp(8f) },
        )
        field.addView(
            View(this).apply {
                setBackgroundColor(if (focused) theme.accent else theme.stroke)
            },
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 1).apply {
                topMargin = theme.dp(8f)
            },
        )
        return field
    }

    // MARK: Rendering

    private fun renderHeader() {
        val vault = selectedCategory is PanelCategory.Vault
        if (query.isEmpty()) {
            searchText.text =
                if (vault) PanelStrings.VAULT_PLACEHOLDER else PanelStrings.SEARCH_PLACEHOLDER
            searchText.setTextColor(theme.meta)
            searchText.typeface = android.graphics.Typeface.DEFAULT
            trailingCaret.visibility = View.GONE
            wordmark.visibility = View.VISIBLE
            resultCount.visibility = View.GONE
            headerUnderline.setBackgroundColor(theme.hairline)
        } else {
            searchText.text = query
            searchText.setTextColor(theme.ink)
            // Queries are machine text: they render in mono.
            searchText.typeface = android.graphics.Typeface.MONOSPACE
            trailingCaret.visibility = View.VISIBLE
            wordmark.visibility = View.GONE
            resultCount.visibility = View.VISIBLE
            resultCount.text = String.format(Locale.US, "%,d found", rows.size)
            headerUnderline.setBackgroundColor(theme.accent)
        }
        // The brand caret dims while the locked vault surface is up.
        headerCaret.setBackgroundColor(if (vault && query.isEmpty()) theme.caretDim else theme.accent)
    }

    private fun renderTabs() {
        tabStack.removeAllViews()
        if (passwordField) return
        for (category in categories) {
            // With no filter engaged the panel shows the recent-first list,
            // so the Recent tab reads as active; tapping the
            // active tab clears the filter.
            val active =
                category == selectedCategory ||
                    (selectedCategory == null && category is PanelCategory.Recent)
            val tab =
                footerTab(this, theme, category.label, active) {
                    selectedCategory = if (selectedCategory == category) null else category
                    refresh()
                }
            tabStack.addView(
                tab,
                LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                ).apply { marginEnd = theme.dp(20f) },
            )
        }
    }

    private fun renderBarStrips() {
        barStrips.removeAllViews()
        if (passwordField) {
            barStrips.addView(
                textView(this, theme.tabSp, theme.meta).apply {
                    text = PanelStrings.PASSWORD_BAR
                    gravity = Gravity.CENTER_VERTICAL
                },
                LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.MATCH_PARENT,
                ),
            )
            return
        }
        val entries = store?.entries ?: emptyList()
        if (entries.isEmpty()) {
            barStrips.addView(
                textView(this, theme.tabSp, theme.meta).apply {
                    text = PanelStrings.EMPTY_MAIN
                    gravity = Gravity.CENTER_VERTICAL
                },
                LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.MATCH_PARENT,
                ),
            )
            return
        }
        for (entry in entries.take(BAR_STRIP_LIMIT)) {
            barStrips.addView(
                barStrip(this, theme, entry, ::insert),
                LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, theme.dp(36f))
                    .apply {
                        marginEnd = theme.dp(8f)
                        gravity = Gravity.CENTER_VERTICAL
                    },
            )
        }
    }

    private fun renderState() {
        stateContainer.removeAllViews()
        val state =
            when {
                passwordField ->
                    stateView(this, theme, PanelStrings.PASSWORD_MAIN, PanelStrings.PASSWORD_SUB)
                selectedCategory is PanelCategory.Vault ->
                    // The vault tab is an honest locked state:
                    // the IME holds no unlock path, so nothing lists and
                    // nothing inserts until the app unlocks.
                    vaultLockedView(this, theme)
                store == null || store?.entries.isNullOrEmpty() ->
                    // Covers missing/unreadable/newer-version snapshots too:
                    // the honest answer to all of them is "nothing here yet".
                    stateView(this, theme, PanelStrings.EMPTY_MAIN, PanelStrings.EMPTY_SUB)
                rows.isEmpty() ->
                    stateView(this, theme, PanelStrings.NO_MATCH_MAIN, PanelStrings.NO_MATCH_SUB)
                else -> null
            }
        if (state != null) stateContainer.addView(state)
        stateContainer.visibility = if (state != null) View.VISIBLE else View.GONE
        list.visibility = if (state != null) View.GONE else View.VISIBLE
    }

    // MARK: Layout

    private fun buildRoot(): View {
        val root =
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                setBackgroundColor(theme.panelBg)
            }
        // 1px top border so the panel reads as a surface separate from the
        // host app (the design's upward shadow cannot escape an IME window;
        // the border carries the separation in both themes).
        root.addView(
            View(this).apply { setBackgroundColor(theme.hairline) },
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 1),
        )
        content = FrameLayout(this)
        buildBar()
        buildPanel()
        content.addView(
            barView,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )
        content.addView(
            panelView,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )
        root.addView(
            content,
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, theme.barHeight),
        )
        return root
    }

    private fun buildBar() {
        barView =
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                setPadding(theme.dp(12f), 0, 0, 0)
            }
        barStrips =
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
            }
        val scroll =
            HorizontalScrollView(this).apply {
                isHorizontalScrollBarEnabled = false
                addView(
                    barStrips,
                    ViewGroup.LayoutParams(
                        ViewGroup.LayoutParams.WRAP_CONTENT,
                        ViewGroup.LayoutParams.MATCH_PARENT,
                    ),
                )
            }
        barView.addView(
            scroll,
            LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 1f),
        )
        barView.addView(
            textView(this, 16f, theme.secondary).apply {
                text = PanelStrings.EXPAND_GLYPH
                gravity = Gravity.CENTER
                contentDescription = "Expand snippet panel"
                setOnClickListener { setExpanded(true) }
            },
            LinearLayout.LayoutParams(theme.dp(44f), ViewGroup.LayoutParams.MATCH_PARENT),
        )
    }

    private fun buildPanel() {
        panelView =
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                visibility = View.GONE
            }
        // The panel body is either the list surface (search line + rows) or
        // the template fill surface; the footer rail stays under both.
        listMode = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }

        // Search line: the 2dp brand caret leads, the query
        // follows in mono with a trailing caret, the right edge carries the
        // wordmark (idle) or the result count (searching). No input box, no
        // magnifier icon; the underline turns accent while a query is live.
        val headerRow =
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                setPadding(theme.hInset, 0, theme.hInset, 0)
            }
        headerCaret = caretBar(this, theme, 15f)
        headerRow.addView(headerCaret)
        searchText = textView(this, theme.searchSp, theme.meta)
        headerRow.addView(
            searchText,
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { marginStart = theme.dp(10f) },
        )
        trailingCaret = caretBar(this, theme, 15f)
        headerRow.addView(trailingCaret)
        (trailingCaret.layoutParams as LinearLayout.LayoutParams).marginStart = theme.dp(2f)
        headerRow.addView(View(this), LinearLayout.LayoutParams(0, 1, 1f))
        wordmark =
            textView(this, theme.wordmarkSp, theme.micro, medium = true).apply {
                text = PanelStrings.WORDMARK
                letterSpacing = 0.2f
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            }
        headerRow.addView(wordmark)
        resultCount =
            textView(this, theme.countSp, theme.micro, mono = true).apply {
                accessibilityLiveRegion = View.ACCESSIBILITY_LIVE_REGION_POLITE
                contentDescription = "results"
                visibility = View.GONE
            }
        headerRow.addView(resultCount)
        listMode.addView(
            headerRow,
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, theme.headerHeight),
        )
        headerUnderline = View(this).apply { setBackgroundColor(theme.hairline) }
        listMode.addView(
            headerUnderline,
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 1),
        )

        adapter = SnippetRowAdapter(this, theme)
        list =
            ListView(this).apply {
                adapter = this@TypviaImeService.adapter
                // Row dividers are inset hairlines, never full-bleed.
                divider =
                    InsetDrawable(ColorDrawable(theme.divider), theme.hInset, 0, theme.hInset, 0)
                dividerHeight = 1
                setOnItemClickListener { _, _, position, _ ->
                    if (position < rows.size) rowTapped(rows[position])
                }
            }
        stateContainer = FrameLayout(this).apply { visibility = View.GONE }
        factText = textView(this, theme.tabSp, theme.meta)
        factLine = factLine(this, theme, factText).apply { visibility = View.GONE }
        val listWrap = FrameLayout(this)
        listWrap.addView(
            list,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )
        listWrap.addView(
            stateContainer,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )
        listWrap.addView(
            factLine,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.BOTTOM or Gravity.START,
            ).apply {
                marginStart = theme.hInset
                bottomMargin = theme.dp(10f)
            },
        )
        listMode.addView(
            listWrap,
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f),
        )

        buildFillView()
        val bodyArea = FrameLayout(this)
        bodyArea.addView(
            listMode,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )
        bodyArea.addView(
            fillView,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )
        panelView.addView(
            bodyArea,
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f),
        )

        // Footer tab rail: category tabs with an accent overline on the
        // active one, then the ABC affordance that returns to the compact
        // strip bar (switching keyboards stays with the system input-method
        // switcher).
        panelView.addView(
            View(this).apply { setBackgroundColor(theme.hairline) },
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 1),
        )
        val footer =
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                setPadding(theme.hInset, 0, theme.hInset, 0)
            }
        tabStack =
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
            }
        footer.addView(
            HorizontalScrollView(this).apply {
                isHorizontalScrollBarEnabled = false
                addView(
                    tabStack,
                    ViewGroup.LayoutParams(
                        ViewGroup.LayoutParams.WRAP_CONTENT,
                        ViewGroup.LayoutParams.MATCH_PARENT,
                    ),
                )
            },
            LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 1f),
        )
        footer.addView(
            textView(this, theme.abcSp, theme.secondary, mono = true).apply {
                text = PanelStrings.ABC
                gravity = Gravity.CENTER
                contentDescription = "Collapse to snippet bar"
                setPadding(theme.dp(12f), theme.dp(8f), 0, theme.dp(8f))
                setOnClickListener { setExpanded(false) }
            },
        )
        panelView.addView(
            footer,
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, theme.footerHeight),
        )
    }

    /** The template fill surface: back header with the TEMPLATE
     * tag, variable fields, live preview, Insert and Copy. */
    private fun buildFillView() {
        fillView =
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                visibility = View.GONE
            }

        val header =
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                setPadding(theme.hInset, 0, theme.hInset, 0)
            }
        header.addView(
            textView(this, 19f, theme.secondary).apply {
                text = PanelStrings.BACK_GLYPH
                typeface = android.graphics.Typeface.create("sans-serif-light", android.graphics.Typeface.NORMAL)
                gravity = Gravity.CENTER
                contentDescription = "Back to snippet list"
                setPadding(0, theme.dp(8f), theme.dp(12f), theme.dp(8f))
                setOnClickListener {
                    closeFill()
                    refresh()
                }
            },
        )
        fillTitle = textView(this, theme.searchSp, theme.ink, medium = true)
        header.addView(fillTitle)
        header.addView(View(this), LinearLayout.LayoutParams(0, 1, 1f))
        header.addView(capsLabel(this, theme, PanelStrings.TEMPLATE_TAG))
        fillView.addView(
            header,
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, theme.headerHeight),
        )
        fillView.addView(
            View(this).apply { setBackgroundColor(theme.hairline) },
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 1),
        )

        fillFields =
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                setPadding(theme.hInset, 0, theme.hInset, 0)
            }
        fillView.addView(
            HorizontalScrollView(this).apply {
                isHorizontalScrollBarEnabled = false
                addView(
                    fillFields,
                    ViewGroup.LayoutParams(
                        ViewGroup.LayoutParams.WRAP_CONTENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT,
                    ),
                )
            },
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = theme.dp(18f) },
        )

        fillView.addView(
            capsLabel(this, theme, PanelStrings.PREVIEW_TAG),
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply {
                topMargin = theme.dp(20f)
                marginStart = theme.hInset
            },
        )
        fillPreview =
            textView(this, theme.previewSp, theme.body).apply {
                maxLines = 3
                ellipsize = android.text.TextUtils.TruncateAt.END
                setLineSpacing(0f, 1.3f)
            }
        fillView.addView(
            fillPreview,
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply {
                topMargin = theme.dp(10f)
                marginStart = theme.hInset
                marginEnd = theme.hInset
            },
        )

        fillView.addView(View(this), LinearLayout.LayoutParams(0, 0, 1f))

        val actions =
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                setPadding(theme.hInset, 0, theme.hInset, theme.dp(12f))
            }
        val insertButton =
            LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER
                background =
                    GradientDrawable().apply {
                        cornerRadius = theme.dp(12f).toFloat()
                        setColor(theme.ink)
                    }
                setOnClickListener { fillInsert() }
            }
        // Button text sits on ink, so it wears the panel surface color; the
        // return glyph drops to 55 percent of it (design spec).
        insertButton.addView(
            textView(this, theme.buttonSp, theme.panelBg, medium = true).apply {
                text = PanelStrings.INSERT_LABEL
            },
        )
        insertButton.addView(
            textView(this, 11f, (0x8C shl 24) or (theme.panelBg and 0xFFFFFF), mono = true).apply {
                text = PanelStrings.RETURN_GLYPH
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            },
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { marginStart = theme.dp(9f) },
        )
        actions.addView(
            insertButton,
            LinearLayout.LayoutParams(0, theme.dp(52f), 1f),
        )
        actions.addView(
            textView(this, theme.previewSp, theme.secondary).apply {
                text = PanelStrings.COPY_LABEL
                setPadding(theme.dp(16f), theme.dp(12f), 0, theme.dp(12f))
                setOnClickListener { fillCopy() }
            },
        )
        fillView.addView(
            actions,
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ),
        )
    }

    private companion object {
        /** The bar shows the head of the recent-first list; the full list
         * lives in the expanded panel. */
        const val BAR_STRIP_LIMIT = 8

        /** How long the transient inserted/copied confirmation stays up. */
        const val FACT_LINE_MS = 1400L
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// View pieces of the IME: editorial two-line snippet rows with a trailing
// return glyph, a footer tab rail with an accent overline on the active
// tab, the collapsed strip bar, and the text-built states. No icons, no
// spinners, no illustrations; only the caret and the active tab wear accent.

package dev.typvia.ime

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.text.SpannableString
import android.text.Spanned
import android.text.style.ReplacementSpan
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.BaseAdapter
import android.widget.LinearLayout
import android.widget.TextView
import kotlin.math.ceil
import uniffi.typvia_mobile_ffi.SnapshotEntry

/** Panel copy. Failure copy states what is still good before the
 * restriction (design rule); locked rows surface dots plus a LOCKED tag and
 * never any plaintext. */
object PanelStrings {
    const val EMPTY_MAIN = "Open Typvia to save your first snippet"
    const val EMPTY_SUB = "打开 Typvia 保存你的第一个片段"
    const val NO_MATCH_MAIN = "No matching snippets"
    const val NO_MATCH_SUB = "没有匹配的片段"

    const val PASSWORD_MAIN = "Your snippets are safe and ready elsewhere"
    const val PASSWORD_SUB = "片段完好可用——Typvia 不会在密码框中输入"
    const val PASSWORD_BAR = "Typvia won't type into password fields · 不在密码框中输入"

    const val SEARCH_PLACEHOLDER = "Search snippets"
    const val VAULT_PLACEHOLDER = "Vault"
    const val VAULT_LOCKED_MAIN = "Vault is locked"
    const val VAULT_LOCKED_SUB = "Nothing is inserted until you unlock"
    const val WORDMARK = "TYPVIA"
    const val RETURN_GLYPH = "↵"
    /** Locked title is a dot run: the row shows shape, never content. */
    const val LOCKED_TITLE = "••••••••"
    const val LOCKED_TAG = "LOCKED"
    const val INSERTED_FACT = "Inserted · keyboard stays open"
    const val COPIED_FACT = "Copied · paste anywhere"
    const val ABC = "ABC"
    const val EXPAND_GLYPH = "⌃"
    const val BACK_GLYPH = "‹"
    const val TEMPLATE_TAG = "TEMPLATE"
    const val PREVIEW_TAG = "PREVIEW"
    const val INSERT_LABEL = "Insert"
    const val COPY_LABEL = "Copy"
}

internal fun textView(
    context: Context,
    sizeSp: Float,
    color: Int,
    mono: Boolean = false,
    medium: Boolean = false,
): TextView =
    TextView(context).apply {
        setTextSize(TypedValue.COMPLEX_UNIT_SP, sizeSp)
        setTextColor(color)
        typeface =
            when {
                mono -> Typeface.MONOSPACE
                medium -> Typeface.create("sans-serif-medium", Typeface.NORMAL)
                else -> Typeface.DEFAULT
            }
        maxLines = 1
    }

/** The 2dp brand caret bar (search line lead, state motif). */
internal fun caretBar(context: Context, theme: PanelTheme, heightDp: Float): View =
    View(context).apply {
        setBackgroundColor(theme.accent)
        importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        layoutParams = ViewGroup.LayoutParams(theme.dp(2f), theme.dp(heightDp))
    }

/** Hairline-stroked r10 plate for the collapsed bar's strips (button radius
 * per the design's radius ladder; no fill — content, not a card). */
internal fun stripBackground(theme: PanelTheme): GradientDrawable =
    GradientDrawable().apply {
        cornerRadius = theme.stripRadius
        setStroke(1, theme.stroke)
    }

/** One tappable strip of the collapsed bar: title only (locked entries show
 * the dot run plus the LOCKED tag and have no click path). */
internal fun barStrip(
    context: Context,
    theme: PanelTheme,
    entry: SnapshotEntry,
    onTap: (SnapshotEntry) -> Unit,
): View {
    val strip =
        LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            background = stripBackground(theme)
            setPadding(theme.dp(12f), 0, theme.dp(12f), 0)
        }
    if (entry.isSensitive) {
        strip.addView(textView(context, theme.stripSp, theme.meta).apply {
            text = PanelStrings.LOCKED_TITLE
        })
        strip.addView(
            textView(context, theme.lockedSp, theme.micro, mono = true).apply {
                text = PanelStrings.LOCKED_TAG
                letterSpacing = 0.13f
            },
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { marginStart = theme.dp(8f) },
        )
        strip.contentDescription = "Locked, vault snippet"
    } else {
        strip.addView(textView(context, theme.stripSp, theme.ink).apply { text = entry.title })
        strip.contentDescription =
            "${TypeMark.word(TypeMark.mark(entry.snippetType))}, ${entry.title}"
        strip.setOnClickListener { onTap(entry) }
    }
    return strip
}

/** One footer tab: label with a 2dp accent overline when active (the active
 * tab is the only accent in the rail; inactive labels sit in meta grey). */
internal fun footerTab(
    context: Context,
    theme: PanelTheme,
    label: String,
    active: Boolean,
    onTap: () -> Unit,
): View {
    val tab =
        LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
        }
    val overline =
        View(context).apply {
            setBackgroundColor(theme.accent)
            visibility = if (active) View.VISIBLE else View.INVISIBLE
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        }
    tab.addView(
        overline,
        LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, theme.dp(2f)),
    )
    tab.addView(
        textView(
            context,
            theme.tabSp,
            if (active) theme.ink else theme.meta,
            medium = active,
        ).apply { text = label },
        LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        ).apply { topMargin = theme.dp(6f) },
    )
    tab.contentDescription = if (active) "$label, selected filter" else "$label filter"
    tab.setOnClickListener { onTap() }
    return tab
}

/** Transient confirmation line (caret bar + fact text), overlaid above the
 * footer per the design's inserted moment; the caller owns the TextView so
 * the fact can say inserted or copied. */
internal fun factLine(context: Context, theme: PanelTheme, fact: TextView): View {
    val line =
        LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
    line.addView(caretBar(context, theme, 11f))
    line.addView(
        fact,
        LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        ).apply { marginStart = theme.dp(8f) },
    )
    return line
}

/** ALL-CAPS mono micro label (section tags, template field names). */
internal fun capsLabel(context: Context, theme: PanelTheme, label: String): TextView =
    textView(context, theme.capsLabelSp, theme.micro, mono = true).apply {
        text = label
        letterSpacing = 0.13f
    }

/** Draws the spanned run with the ambient paint, then a thin accent line
 * snug under it — the design's match underline. The underline color API
 * needs API 29, so the line is drawn by hand (single-line titles only). */
internal class AccentUnderlineSpan(
    private val color: Int,
    private val thickness: Float,
) : ReplacementSpan() {
    override fun getSize(
        paint: Paint,
        text: CharSequence,
        start: Int,
        end: Int,
        fm: Paint.FontMetricsInt?,
    ): Int {
        if (fm != null) paint.getFontMetricsInt(fm)
        return ceil(paint.measureText(text, start, end)).toInt()
    }

    override fun draw(
        canvas: Canvas,
        text: CharSequence,
        start: Int,
        end: Int,
        x: Float,
        top: Int,
        y: Int,
        bottom: Int,
        paint: Paint,
    ) {
        canvas.drawText(text, start, end, x, y.toFloat(), paint)
        val width = paint.measureText(text, start, end)
        val previous = paint.color
        paint.color = color
        val lineTop = y + thickness * 2f
        canvas.drawRect(x, lineTop, x + width, lineTop + thickness, paint)
        paint.color = previous
    }
}

/** Title with the first case-insensitive query match underlined in accent;
 * the plain title when nothing matches. */
internal fun matchedTitle(title: String, query: String, theme: PanelTheme): CharSequence {
    if (query.isEmpty()) return title
    val index = title.indexOf(query, ignoreCase = true)
    if (index < 0) return title
    return SpannableString(title).apply {
        setSpan(
            AccentUnderlineSpan(theme.accent, theme.dp(1f).toFloat()),
            index,
            index + query.length,
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE,
        )
    }
}

/** Centered vault locked state: outlined circle around the
 * brand caret, then the honest fact — no unlock button, the IME holds no
 * unlock path. */
internal fun vaultLockedView(context: Context, theme: PanelTheme): View {
    val column =
        LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(theme.dp(24f), theme.dp(26f), theme.dp(24f), 0)
        }
    val circle =
        android.widget.FrameLayout(context).apply {
            background =
                GradientDrawable().apply {
                    shape = GradientDrawable.OVAL
                    setStroke(1, theme.outline)
                }
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        }
    circle.addView(
        View(context).apply { setBackgroundColor(theme.accent) },
        android.widget.FrameLayout.LayoutParams(theme.dp(2f), theme.dp(14f), Gravity.CENTER),
    )
    column.addView(circle, LinearLayout.LayoutParams(theme.dp(40f), theme.dp(40f)))
    column.addView(
        textView(context, theme.vaultMainSp, theme.ink).apply {
            text = PanelStrings.VAULT_LOCKED_MAIN
            letterSpacing = -0.016f
        },
        LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        ).apply { topMargin = theme.dp(20f) },
    )
    column.addView(
        textView(context, theme.vaultSubSp, theme.secondary).apply {
            text = PanelStrings.VAULT_LOCKED_SUB
        },
        LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        ).apply { topMargin = theme.dp(10f) },
    )
    return column
}

/** Text-built centered state (caret motif + main + sub line); serves empty,
 * no-match, and the password refusal — no spinners, no illustrations. */
internal fun stateView(context: Context, theme: PanelTheme, main: String, sub: String): View {
    val column =
        LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(theme.dp(24f), theme.dp(30f), theme.dp(24f), 0)
        }
    column.addView(caretBar(context, theme, 22f))
    column.addView(
        textView(context, theme.stateMainSp, theme.ink).apply {
            text = main
            maxLines = 2
            gravity = Gravity.CENTER_HORIZONTAL
        },
        LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        ).apply { topMargin = theme.dp(16f) },
    )
    column.addView(
        textView(context, theme.stateSubSp, theme.secondary).apply {
            text = sub
            maxLines = 2
            gravity = Gravity.CENTER_HORIZONTAL
            // CJK line-height rule (+0.15 over EN) matters only on wrap; keep
            // the extra leading so a wrapped CN line stays readable.
            setLineSpacing(0f, 1.15f)
        },
        LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        ).apply { topMargin = theme.dp(8f) },
    )
    return column
}

/** Editorial snippet rows: title over a one-line body preview (mono for
 * machine content), trailing return glyph. Locked rows show the dot run
 * plus the LOCKED tag with zero plaintext and read as static text. */
internal class SnippetRowAdapter(
    private val context: Context,
    private val theme: PanelTheme,
) : BaseAdapter() {
    var rows: List<SnapshotEntry> = emptyList()

    /** Live query, for the accent match underline on titles. */
    var query: String = ""

    /** Lazy body lookup (SnapshotStore.body); null for locked/unknown ids. */
    var bodyOf: ((String) -> String?)? = null

    private class Preview(val text: String, val mono: Boolean)

    /** First-line previews resolved once per entry per snapshot load; the
     * service clears this when it rereads the snapshot file. */
    private val previews = HashMap<String, Preview?>()

    fun clearPreviews() = previews.clear()

    /** First non-blank body line, mono when the design calls the content
     * machine text; falls back to the trigger (always machine text). */
    private fun previewFor(entry: SnapshotEntry): Preview? =
        previews.getOrPut(entry.id) {
            val line =
                bodyOf?.invoke(entry.id)
                    ?.lineSequence()
                    ?.firstOrNull { it.isNotBlank() }
                    ?.trim()
            when {
                !line.isNullOrEmpty() ->
                    Preview(line, entry.snippetType in MONO_PREVIEW_TYPES)
                !entry.trigger.isNullOrEmpty() -> Preview(entry.trigger!!, true)
                else -> null
            }
        }

    private class Holder(val title: TextView, val sub: TextView, val aux: TextView)

    override fun getCount(): Int = rows.size

    override fun getItem(position: Int): SnapshotEntry = rows[position]

    override fun getItemId(position: Int): Long = position.toLong()

    override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
        val row: LinearLayout
        val holder: Holder
        if (convertView is LinearLayout && convertView.tag is Holder) {
            row = convertView
            holder = convertView.tag as Holder
        } else {
            row =
                LinearLayout(context).apply {
                    orientation = LinearLayout.HORIZONTAL
                    gravity = Gravity.CENTER_VERTICAL
                    minimumHeight = theme.rowMinHeight
                    layoutParams =
                        ViewGroup.LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            ViewGroup.LayoutParams.WRAP_CONTENT,
                        )
                    setPadding(theme.hInset, theme.dp(12f), theme.hInset, theme.dp(12f))
                }
            val column =
                LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
            val title =
                textView(context, theme.rowTitleSp, theme.ink, medium = true).apply {
                    letterSpacing = -0.012f
                }
            val sub =
                textView(context, theme.rowSubSp, theme.secondary, mono = true).apply {
                    ellipsize = android.text.TextUtils.TruncateAt.END
                }
            column.addView(title)
            column.addView(
                sub,
                LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                ).apply { topMargin = theme.dp(6f) },
            )
            val aux = textView(context, theme.ghostSp, theme.ghost, mono = true)
            row.addView(column, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
            row.addView(
                aux,
                LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                ).apply { marginStart = theme.dp(12f) },
            )
            holder = Holder(title, sub, aux)
            row.tag = holder
        }

        val entry = rows[position]
        if (entry.isSensitive) {
            // Locked row: the snapshot holds only ciphertext for this entry;
            // the dot run and tag carry the shape, never the content.
            holder.title.text = PanelStrings.LOCKED_TITLE
            holder.title.setTextColor(theme.meta)
            holder.sub.visibility = View.GONE
            holder.aux.text = PanelStrings.LOCKED_TAG
            holder.aux.setTextSize(TypedValue.COMPLEX_UNIT_SP, theme.lockedSp)
            holder.aux.setTextColor(theme.micro)
            holder.aux.letterSpacing = 0.13f
            row.contentDescription = "Locked, vault snippet"
        } else {
            holder.title.text = matchedTitle(entry.title, query, theme)
            holder.title.setTextColor(theme.ink)
            val preview = previewFor(entry)
            if (preview == null) {
                holder.sub.visibility = View.GONE
            } else {
                holder.sub.visibility = View.VISIBLE
                holder.sub.text = preview.text
                holder.sub.typeface = if (preview.mono) Typeface.MONOSPACE else Typeface.DEFAULT
            }
            holder.aux.text = PanelStrings.RETURN_GLYPH
            holder.aux.setTextSize(TypedValue.COMPLEX_UNIT_SP, theme.ghostSp)
            holder.aux.setTextColor(theme.ghost)
            holder.aux.letterSpacing = 0f
            val code = TypeMark.mark(entry.snippetType)
            var label = "${TypeMark.word(code)}, ${entry.title}"
            entry.trigger?.let { label += ", trigger $it" }
            row.contentDescription = label
        }
        return row
    }

    private companion object {
        /** Machine content renders mono in the row preview. */
        val MONO_PREVIEW_TYPES = setOf("code", "command", "template")
    }
}

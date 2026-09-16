// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// The pieces the bench is drawn from: the search line, the sorts band, the
// sheets, the function row, and the three faces that replace the run of
// sheets — nothing matched, nothing to reach, and the ink room.
//
// No icons, no spinners, no illustrations. The caret and the trigger word are
// the only things in accent, and a sheet is lifted by its own paper rather
// than filled with a colour.

package dev.typvia.ime

import android.content.Context
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.text.SpannableString
import android.text.Spanned
import android.text.TextUtils
import android.text.style.UnderlineSpan
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout
import android.widget.TextView
import dev.typvia.mobile.ui.Translator

/**
 * Panel copy, as inline pairs.
 *
 * Exactly one half of each pair is ever rendered — a keyboard that prints both
 * languages at once is the mixed-language surface the delivery forbids, and
 * this panel used to be one.
 */
class BenchCopy(private val tr: Translator) {
    val searchPlaceholder = tr("Search snippets", "搜索片段")
    val allKinds = tr("All", "全部")
    val abc = tr("ABC", "ABC")
    val recentScope = tr("Recent", "最近")
    val starredScope = tr("Favorites", "收藏")
    val matchScope = tr("Matches", "匹配")
    val backspace = "⌫"
    val undo = tr("Undo", "撤销")
    val justUsed = tr("just used", "刚用过")

    fun matchCount(count: Int): String =
        if (count == 1) tr("1 match", "1 枚匹配") else tr("$count matches", "$count 枚匹配")

    fun inserted(title: String): String = tr("Typed “$title”", "已插入「$title」")

    val noMatchTitle = tr("Nothing matches that word yet.", "这个词还没有对应的片段。")
    val noKindTitle = tr("Nothing of that kind here yet.", "这一类还没有东西。")
    val noMatchBody = tr(
        "Try another wording, or write it in Typvia.",
        "换个说法,或者去 Typvia 里写一枚。",
    )
    val createInApp = tr("Open Typvia", "去 Typvia 新建")
    val clear = tr("Clear", "清空")

    val unreachableTitle = tr("Nothing to reach yet.", "还没有可以拿的东西。")
    val unreachableBody = tr(
        "Save a snippet in Typvia and it will be waiting here.",
        "在 Typvia 里存一枚,它就会在这里等着。",
    )

    val passwordTitle = tr(
        "Your snippets are safe, and ready everywhere else.",
        "片段都好好的,在别处照常可用。",
    )
    val passwordBody = tr(
        "Typvia does not type into password fields.",
        "Typvia 不会在密码框里输入。",
    )

    fun secretBody(title: String): String = tr(
        "“$title” goes straight into the field. It is never shown here, and never on the clipboard.",
        "「$title」会直接落进输入框,不在键盘里显示,也不进剪贴板。",
    )

    /** The deviation this platform actually has: the panel holds no way in. */
    val secretShut = tr(
        "Unlock the vault in Typvia first, then come back.",
        "先在 Typvia 里打开保险库,再回到这里。",
    )
    val backToBench = tr("Back to the bench", "回到台面")
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
        typeface = when {
            mono -> Typeface.MONOSPACE
            medium -> Typeface.create("sans-serif-medium", Typeface.NORMAL)
            else -> Typeface.DEFAULT
        }
        maxLines = 1
    }

/** The caret: a bar of accent, and the one thing that leads a search line. */
internal fun caret(context: Context, theme: PanelTheme, heightDp: Float, color: Int): View =
    View(context).apply {
        setBackgroundColor(color)
        importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        layoutParams = ViewGroup.LayoutParams(theme.caretWidth, theme.dp(heightDp))
    }

internal fun rule(context: Context, theme: PanelTheme): View =
    View(context).apply {
        setBackgroundColor(theme.hairline)
        importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
    }

/**
 * One lead in the sorts band. The active one is ink; the rest are the third
 * grey; one with nothing behind it right now dims rather than leaving.
 */
internal fun sortLead(
    context: Context,
    theme: PanelTheme,
    label: String,
    sort: BenchSort,
    onTap: () -> Unit,
): View =
    textView(context, theme.markSp, if (sort.isActive) theme.ink else theme.ink3, mono = true).apply {
        text = label
        letterSpacing = 0.08f
        gravity = Gravity.CENTER
        alpha = if (sort.hasHits) 1f else theme.dimmedAlpha
        contentDescription = sort.mark?.let { TypeMark.word(it) } ?: label
        setPadding(theme.dp(10f), 0, theme.dp(10f), 0)
        // A dimmed lead is still a lead: it says the kind exists, and tapping
        // it is how a reader finds out that nothing of it matches right now.
        setOnClickListener { onTap() }
    }

/** The two-letter mark a sheet wears. Only the secret one carries a ground. */
internal fun typeMark(context: Context, theme: PanelTheme, mark: String, onVault: Boolean): View =
    textView(
        context,
        theme.markSp,
        if (mark == "SC") theme.paper else if (onVault) theme.vaultInk2 else theme.ink3,
        mono = true,
    ).apply {
        text = mark
        letterSpacing = 0.08f
        gravity = Gravity.CENTER
        contentDescription = TypeMark.word(mark)
        if (mark == "SC") {
            background = GradientDrawable().apply {
                cornerRadius = theme.markRadius
                setColor(theme.ink)
            }
            setPadding(theme.dp(4f), theme.dp(1f), theme.dp(4f), theme.dp(1f))
        }
    }

/**
 * One sheet.
 *
 * The lead sheet is wider and shows two lines of what it will type; the rest
 * carry a title and their trigger word. A sheet that was just used steps back
 * to 55 percent and says so, rather than vanishing from under the thumb that
 * tapped it.
 */
internal fun benchTile(
    context: Context,
    theme: PanelTheme,
    copy: BenchCopy,
    tile: BenchTile,
    query: String,
    preview: String?,
    onTap: (BenchTile) -> Unit,
): View {
    val sheet = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        background = GradientDrawable().apply {
            cornerRadius = theme.tileRadius
            setColor(theme.carrier)
        }
        setPadding(theme.dp(12f), theme.dp(12f), theme.dp(12f), theme.dp(12f))
        alpha = if (tile.isSpent) theme.spentAlpha else 1f
        contentDescription = "${TypeMark.word(tile.mark)}, ${tile.title}"
        setOnClickListener { onTap(tile) }
    }
    sheet.addView(
        typeMark(context, theme, tile.mark, onVault = false),
        // Width to the mark itself: a two-letter mark stretched across the
        // sheet stops being a mark and becomes a heading.
        LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        ),
    )
    sheet.addView(
        textView(context, theme.titleSp, theme.ink, medium = true).apply {
            text = tile.title
            maxLines = 2
            ellipsize = TextUtils.TruncateAt.END
        },
        LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        ).apply { topMargin = theme.dp(8f) },
    )
    sheet.addView(View(context), LinearLayout.LayoutParams(0, 0, 1f))
    val footnote = when {
        tile.isSpent -> textView(context, theme.monoSp, theme.ink3).apply { text = copy.justUsed }
        tile.trigger != null ->
            textView(context, theme.monoSp, theme.accent, mono = true).apply {
                // The typed head is underlined, never highlighted: a coloured
                // block behind text is a search engine's idea of emphasis.
                text = underlineTyped(tile.trigger, query)
            }
        else -> null
    }
    if (footnote != null) {
        sheet.addView(
            footnote,
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = theme.dp(6f) },
        )
    }
    if (tile.isLead && preview != null) {
        sheet.addView(
            textView(context, theme.bodySp, theme.ink2).apply {
                text = preview
                maxLines = 2
                ellipsize = TextUtils.TruncateAt.END
                setLineSpacing(0f, 1.3f)
                // The words themselves are read out by the title; a preview
                // read aloud twice is noise.
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            },
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = theme.dp(6f) },
        )
    }
    return sheet
}

private fun underlineTyped(trigger: String, query: String): CharSequence {
    val typed = query.trim()
    if (typed.isEmpty() || !trigger.startsWith(typed, ignoreCase = true)) return trigger
    return SpannableString(trigger).apply {
        setSpan(UnderlineSpan(), 0, typed.length, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
    }
}

/** One item of the function row. */
internal fun functionItem(
    context: Context,
    theme: PanelTheme,
    label: String,
    active: Boolean,
    description: String,
    onTap: () -> Unit,
): View =
    textView(context, theme.monoSp, if (active) theme.ink else theme.ink3, mono = true).apply {
        text = label
        letterSpacing = 0.08f
        gravity = Gravity.CENTER
        contentDescription = description
        minWidth = theme.dp(48f)
        setPadding(theme.dp(12f), 0, theme.dp(12f), 0)
        setOnClickListener { onTap() }
    }

/**
 * A face that replaces the run of sheets.
 *
 * Built out of a caret, a line of ink and a line of the second grey — the same
 * way every other empty state in this product is built.
 */
internal fun benchState(
    context: Context,
    theme: PanelTheme,
    title: String,
    body: String,
    accent: Int = theme.accent,
    ink: Int = theme.ink,
    ink2: Int = theme.ink2,
): LinearLayout {
    val face = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        gravity = Gravity.CENTER_VERTICAL
        setPadding(theme.screenPadding, 0, theme.screenPadding, 0)
    }
    val head = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
    }
    head.addView(caret(context, theme, 15f, accent))
    head.addView(
        textView(context, theme.titleSp, ink, medium = true).apply {
            text = title
            maxLines = 2
        },
        LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        ).apply { marginStart = theme.dp(10f) },
    )
    face.addView(head)
    face.addView(
        textView(context, theme.bodySp, ink2).apply {
            text = body
            maxLines = 3
            setLineSpacing(0f, 1.3f)
        },
        LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        ).apply { topMargin = theme.dp(8f) },
    )
    return face
}

/** A text action under a state. Never a filled button, never an icon. */
internal fun stateAction(
    context: Context,
    theme: PanelTheme,
    label: String,
    color: Int,
    onTap: () -> Unit,
): View =
    textView(context, theme.bodySp, color, medium = true).apply {
        text = label
        gravity = Gravity.CENTER_VERTICAL
        minHeight = theme.dp(44f)
        setPadding(0, 0, theme.dp(18f), 0)
        setOnClickListener { onTap() }
    }

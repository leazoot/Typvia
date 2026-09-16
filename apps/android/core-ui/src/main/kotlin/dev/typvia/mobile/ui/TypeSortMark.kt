// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

/**
 * The eight kinds a snippet can be, as the two-letter marks the whole product
 * names them by — the same two letters in the library, the keyboard, the
 * editor and the share sheet.
 *
 * No per-kind colours: a kind is a pair of letters in a square, and only the
 * secret one is filled.
 */
enum class TypeSort(val code: String, val coreType: String) {
    Text("TX", "text"),
    Code("CD", "code"),
    Command("CM", "command"),
    Prompt("PR", "prompt"),
    Template("TP", "template"),
    Secret("SC", "sensitive"),
    AiAction("AI", "ai_action"),
    Link("LK", "link"),
    ;

    /** Only the secret sort is filled and reversed. */
    val isReversed: Boolean get() = this == Secret

    fun name(tr: Translator): String = when (this) {
        Text -> tr("Text", "文本")
        Code -> tr("Code", "代码")
        Command -> tr("Commands", "命令")
        Prompt -> tr("Prompts", "提示词")
        Template -> tr("Templates", "模板")
        Secret -> tr("Secrets", "密钥")
        AiAction -> tr("AI actions", "AI 动作")
        Link -> tr("Links", "链接")
    }

    companion object {
        /** The core's word for a kind, read back. An unknown one is not a kind. */
        fun ofCoreType(value: String): TypeSort? = entries.firstOrNull { it.coreType == value }
    }
}

/** The sizes the frames draw a mark at. */
enum class TypeSortSize(val side: androidx.compose.ui.unit.Dp) {
    Compact(22.dp),
    Row(26.dp),
    Key(34.dp),
}

/**
 * A kind, drawn.
 *
 * @param inverted the mark for the kind currently chosen — filled rather than
 *   outlined. It is the same inversion the secret sort wears permanently.
 */
@Composable
fun TypeSortMark(
    sort: TypeSort,
    modifier: Modifier = Modifier,
    size: TypeSortSize = TypeSortSize.Row,
    inverted: Boolean = false,
    accessibilityLabel: String? = null,
) {
    val filled = inverted || sort.isReversed
    val ink = Paper.ink
    val language = LocalTranslator.current.language
    Box(
        modifier = modifier
            .size(size.side)
            .then(
                if (filled) {
                    Modifier.background(ink, RoundedCornerShape(Tokens.Radius.sort))
                } else {
                    Modifier.border(1.5.dp, ink, RoundedCornerShape(Tokens.Radius.sort))
                },
            )
            .then(
                // The two letters are an abbreviation a screen reader should
                // not spell out: it reads the whole word for the kind.
                accessibilityLabel?.let { label ->
                    Modifier.semantics { contentDescription = label }
                } ?: Modifier,
            ),
        contentAlignment = Alignment.Center,
    ) {
        FixedTypeScale {
            Text(
                text = sort.code,
                style = TypviaType.SortMark.style(language),
                color = if (filled) Paper.base else ink,
            )
        }
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import dev.typvia.mobile.ui.LocalRoom
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.TypviaType
import dev.typvia.mobile.ui.UiPreference

/**
 * Settings, as an editorial contents page.
 *
 * Numbers stand in the left margin and double as the navigation; each chapter
 * is a whole page rather than a folding panel; there is not one icon in the
 * room. The right column carries what is currently true, so the page answers
 * the question before it is opened.
 */
@Composable
fun SettingsScreen(
    summary: SettingsSummary?,
    /**
     * The reader's language choice, which is one of the values this page
     * prints. Required rather than defaulted: a page that quietly falls back
     * to "follows the phone" would report a setting it was never given.
     */
    language: UiPreference.Language,
    isUnreadable: Boolean = false,
    /** Opens a chapter that has a page behind it. Null for the ones that do not. */
    onOpenChapter: ((SettingsChapter) -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Paper.base)
            .verticalScroll(rememberScrollState())
            .padding(horizontal = Tokens.Space.screenPadding),
    ) {
        Text(
            text = tr("SETTINGS", "设置"),
            style = TypviaType.MonoLabel.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = 14.dp, bottom = SettingsMetrics.headGap),
        )
        for ((index, chapter) in SettingsChapter.entries.withIndex()) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .then(
                        // Every chapter has a page behind it now. This used to
                        // name them one at a time, which was right while some
                        // rows were print only — a row that looks like a way in
                        // and does nothing is worse than one that plainly is
                        // not one. The list is the enum again.
                        if (onOpenChapter == null) {
                            Modifier
                        } else {
                            Modifier.clickable { onOpenChapter(chapter) }
                        },
                    )
                    .padding(vertical = SettingsMetrics.rowGap),
            ) {
                Text(
                    text = "%02d".format(index + 1),
                    style = TypviaType.Mono.style(tr.language),
                    color = Paper.ink3,
                    modifier = Modifier.padding(end = SettingsMetrics.numberGap),
                )
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = chapter.title(tr),
                        style = TypviaType.SectionTitle.style(tr.language),
                        color = Paper.ink,
                    )
                    Text(
                        text = chapter.blurb(tr),
                        style = TypviaType.Caption.style(tr.language),
                        color = Paper.ink2,
                    )
                }
                Text(
                    // What is currently true, never a guess: an unread value
                    // is absent rather than assumed, and a value that could
                    // not be read says so once rather than pretending to be
                    // still on its way.
                    text = summary?.let { chapter.value(it, language, tr) }
                        ?: if (isUnreadable) tr("unread", "读不到") else "",
                    style = TypviaType.Mono.style(tr.language),
                    color = Paper.ink3,
                )
            }
        }
    }
}

object SettingsMetrics {
    val headGap = 26.dp
    val rowGap = 14.dp
    val numberGap = 14.dp

    /** Above a field's label, and between its label, its line and its rule. */
    val fieldGap = 18.dp
    val fieldInnerGap = 8.dp
}

/**
 * One line to type on: a caps mono label, the line itself, and a hairline
 * under it. No box — the rule is the field.
 *
 * One definition, for the same reason [PaperVerb] is one: the data chapter
 * grew its own and the AI chapter was about to grow a second, and two copies
 * of "how a field looks" is two chances for one of them to forget that a
 * secret has to declare itself to the platform.
 *
 * @param isSecret keeps the keyboard from learning what is typed here, and
 * keeps it off the screen. Anything that is a credential passes true.
 */
@Composable
fun PaperField(
    label: String,
    value: String,
    isSecret: Boolean = false,
    onValueChange: (String) -> Unit,
) {
    val tr = LocalTranslator.current
    Text(
        text = label,
        style = TypviaType.MonoLabel.style(tr.language),
        color = Paper.ink3,
        modifier = Modifier.padding(top = SettingsMetrics.fieldGap),
    )
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        singleLine = true,
        visualTransformation =
            if (isSecret) PasswordVisualTransformation() else VisualTransformation.None,
        keyboardOptions = KeyboardOptions(
            keyboardType = if (isSecret) KeyboardType.Password else KeyboardType.Text,
            autoCorrectEnabled = false,
        ),
        textStyle = TypviaType.Mono.style(tr.language).copy(color = Paper.ink),
        cursorBrush = SolidColor(LocalRoom.current.accent),
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = SettingsMetrics.fieldInnerGap),
    )
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = SettingsMetrics.fieldInnerGap)
            .height(Tokens.Line.hairlineWidth)
            .background(Paper.rule),
    )
}

/**
 * A verb on ordinary paper: ink text, never a filled button, dimmed rather
 * than hidden when there is nothing it can do.
 *
 * One definition, because the settings chapters each grew their own and three
 * copies of "how a verb looks" is three chances for one of them to drift.
 */

@Composable
fun PaperVerb(
    label: String,
    enabled: Boolean = true,
    isPrimary: Boolean = false,
    onTap: () -> Unit,
) {
    val tr = LocalTranslator.current
    val accent = LocalRoom.current.accent
    // A key action is a shape, not a word among words. The delivery draws
    // verbs as plain text and this product did exactly that — and the reader
    // could not tell, on any screen, what was a control and what was a
    // sentence. So the one act each page exists for gets a fill; everything
    // else stays a word, because a page where everything is a button is the
    // same problem the other way round.
    val shape = RoundedCornerShape(Tokens.Radius.control)
    Text(
        text = label,
        style = TypviaType.SectionTitle.style(tr.language),
        color = when {
            !enabled && isPrimary -> Paper.ink3
            !enabled -> Paper.ink3
            // On a fill, the type is the paper it sits on: the accents are
            // dark enough that ink on ink would not read.
            isPrimary -> Paper.base
            else -> Paper.ink
        },
        modifier = Modifier
            .padding(top = Tokens.Space.group)
            .then(if (isPrimary) Modifier.background(if (enabled) accent else Paper.carrier, shape) else Modifier)
            .then(if (enabled) Modifier.clickable(onClick = onTap) else Modifier)
            .heightIn(min = Tokens.Hit.minimum)
            .then(if (isPrimary) Modifier.padding(horizontal = VerbMetrics.padX) else Modifier)
            .wrapContentHeight(),
    )
}

object VerbMetrics {
    val padX = 18.dp
}

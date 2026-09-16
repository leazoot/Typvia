// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import dev.typvia.mobile.ui.Beat
import dev.typvia.mobile.ui.Curve
import dev.typvia.mobile.ui.LocalReduceMotion
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.TypviaType
import dev.typvia.mobile.ui.UiPreference

/**
 * What the look-and-language chapter says, as values.
 *
 * The two concrete languages are named in themselves rather than translated:
 * somebody hunting for Chinese in an English interface is looking for 中文, and
 * a row reading "Chinese" is the one word they are not scanning for.
 */
object AppearanceChapterCopy {
    fun name(choice: UiPreference.Language, tr: Translator): String = when (choice) {
        UiPreference.Language.System -> tr("Follow the phone", "跟随手机")
        UiPreference.Language.En -> "English"
        UiPreference.Language.Zh -> "中文"
    }

    fun name(choice: UiPreference.Appearance, tr: Translator): String = when (choice) {
        UiPreference.Appearance.System -> tr("Follow the phone", "跟随手机")
        UiPreference.Appearance.Light -> tr("Light", "浅色")
        UiPreference.Appearance.Dark -> tr("Dark", "深色")
    }

    /**
     * The right column of the contents page.
     *
     * It reports the language rather than the theme because that is the one a
     * reader comes to this row looking for; the theme is visible from where
     * they are standing.
     */
    fun contentsValue(language: UiPreference.Language, tr: Translator): String = when (language) {
        UiPreference.Language.System -> tr("follows the phone", "跟随手机")
        UiPreference.Language.En -> "English"
        UiPreference.Language.Zh -> "中文"
    }
}

/**
 * Look and language: the one chapter where a setting is chosen rather than
 * reported.
 *
 * Text size is deliberately not among the choices — see the sentence at the
 * foot of the page, which says so to the reader rather than only here.
 */
@Composable
fun AppearanceChapterScreen(
    preferences: UiPreferences,
    onClose: () -> Unit,
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
            text = tr("Back", "返回"),
            style = TypviaType.BodyS.style(tr.language),
            color = Paper.ink2,
            modifier = Modifier
                .padding(top = 14.dp, bottom = SettingsMetrics.headGap)
                .clickable(onClick = onClose),
        )
        Text(
            text = "05",
            style = TypviaType.Title1.style(tr.language),
            color = Paper.ink3,
        )
        Text(
            text = tr("Look & language", "外观与语言"),
            style = TypviaType.Title2.style(tr.language),
            color = Paper.ink,
            modifier = Modifier.padding(top = SettingsMetrics.rowGap),
        )

        Group(tr("INTERFACE LANGUAGE", "界面语言")) {
            for (option in UiPreference.Language.entries) {
                Choice(
                    label = AppearanceChapterCopy.name(option, tr),
                    isChosen = preferences.language == option,
                ) { preferences.language = option }
            }
        }
        Group(tr("APPEARANCE", "外观")) {
            for (option in UiPreference.Appearance.entries) {
                Choice(
                    label = AppearanceChapterCopy.name(option, tr),
                    isChosen = preferences.appearance == option,
                ) { preferences.appearance = option }
            }
        }

        // Said outright rather than tucked away: a reader has no way to know
        // the other surfaces hear this at all.
        Text(
            text = tr(
                "The keyboard follows this too. The share sheet is still English only.",
                "键盘也跟着这里走。分享面板目前只有英文。",
            ),
            style = TypviaType.Mono.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = Tokens.Space.group),
        )
    }
}

/**
 * One group of choices: a caps mono label with its options stacked under it.
 *
 * Stacked rather than laid side by side, because three words do not fit on one
 * line at the largest system type size — the same shape that had to be fixed
 * in the room bar.
 */
@Composable
private fun Group(label: String, content: @Composable () -> Unit) {
    val tr = LocalTranslator.current
    Text(
        text = label,
        style = TypviaType.MonoLabel.style(tr.language),
        color = Paper.ink3,
        modifier = Modifier.padding(top = Tokens.Space.section),
    )
    content()
}

/**
 * One option. The chosen one gets a slip of paper under it; the others are
 * only words — the weight of a choice is shown by material, not by a control.
 *
 * This is the AI chapter's own way of putting a choice, reused rather than
 * reinvented: the same plate, the same beat, the same platform floor for how
 * small a thing may be and still be tapped.
 */
@Composable
private fun Choice(label: String, isChosen: Boolean, onTap: () -> Unit) {
    val tr = LocalTranslator.current
    val reduceMotion = LocalReduceMotion.current
    val plate by animateColorAsState(
        targetValue = if (isChosen) Paper.carrier else Color.Transparent,
        animationSpec = if (reduceMotion) {
            tween(Tokens.Motion.REDUCED_CROSS_FADE)
        } else {
            tween(Beat.State.durationMs, easing = Curve.enter)
        },
        label = "plate",
    )
    Text(
        text = label,
        style = TypviaType.SectionTitle.style(tr.language),
        color = if (isChosen) Paper.ink else Paper.ink2,
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = SettingsMetrics.rowGap)
            .background(plate, RoundedCornerShape(Tokens.Radius.control))
            .clickable(onClick = onTap)
            .semantics(mergeDescendants = true) { selected = isChosen }
            .heightIn(min = Tokens.Hit.minimum)
            .padding(SettingsMetrics.numberGap),
    )
}

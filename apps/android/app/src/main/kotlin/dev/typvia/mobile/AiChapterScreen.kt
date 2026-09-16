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
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import dev.typvia.mobile.ui.Beat
import dev.typvia.mobile.ui.Curve
import dev.typvia.mobile.ui.LocalReduceMotion
import dev.typvia.mobile.ui.LocalRoom
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Room
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.TypviaType

/**
 * The AI chapter: which machine an action's text would be handed to.
 *
 * The room turns clay here — the room colour follows the content rather than
 * the navigation — and the chosen option stands on a paper plate while the
 * others are only words.
 *
 * The API key is typed on this page and goes straight to the platform's key
 * store behind the bridge. It is held for exactly as long as the keystroke
 * that types it and the call that stores it: never written to the database,
 * never logged, never carried by a sync round, and dropped from this screen
 * the moment it is handed over.
 */
@Composable
fun AiChapterScreen(
    engine: AiEngine,
    refusal: AiRefusal?,
    isWorking: Boolean,
    onChoose: (AiEngine, String, String, String?) -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
    var model by remember { mutableStateOf("") }
    var baseUrl by remember { mutableStateOf("") }
    var apiKey by remember { mutableStateOf("") }
    /**
     * The engine being looked at, which is not yet the engine in effect.
     *
     * Tapping a machine opens its fields; it is the verb underneath that puts
     * it into effect. A tap that silently switched engines would change where
     * the reader's text goes without asking for the one thing that decides it.
     */
    var pending by remember { mutableStateOf<AiEngine?>(null) }
    // What is in effect has changed under this page — the fields belong to
    // whatever that is now, not to what was being looked at before.
    LaunchedEffect(engine) { pending = null }
    val shown = pending ?: engine

    CompositionLocalProvider(LocalRoom provides Room.Ai) {
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
                text = "03",
                style = TypviaType.Title1.style(tr.language),
                color = Paper.ink3,
            )
            Text(
                text = tr("AI engine", "AI 引擎"),
                style = TypviaType.Title2.style(tr.language),
                color = Paper.ink,
                modifier = Modifier.padding(top = SettingsMetrics.rowGap),
            )
            Text(
                text = tr(
                    "An AI action hands the text you selected to some model. Which one you pick decides how far that text travels.",
                    "AI 动作要把你选中的文字交给某个模型。选哪一个,决定了这段文字会走多远。",
                ),
                style = TypviaType.BodyS.style(tr.language),
                color = Paper.ink2,
                modifier = Modifier.padding(top = SettingsMetrics.rowGap),
            )

            for (candidate in AiEngine.entries) {
                Choice(
                    engine = candidate,
                    isChosen = candidate == shown,
                    // Only the one in effect is stated as such. A plate under
                    // something merely being looked at would report a setting
                    // that is not in force.
                    isInEffect = candidate == engine,
                ) {
                    // Turning it off needs nothing typed, so it happens on the
                    // tap. The other two open their fields and wait for the
                    // verb underneath.
                    if (candidate == AiEngine.Off) {
                        apiKey = ""
                        onChoose(candidate, "", "", null)
                    } else {
                        pending = candidate
                    }
                }
                if (candidate == shown && candidate != AiEngine.Off) {
                    Column(modifier = Modifier.padding(start = SettingsMetrics.numberGap)) {
                        PaperField(
                            label = tr("MODEL NAME", "模型名"),
                            value = model,
                            onValueChange = { model = it },
                        )
                        if (candidate == AiEngine.OwnKey) {
                            PaperField(
                                label = tr("ENDPOINT", "接口地址"),
                                value = baseUrl,
                                onValueChange = { baseUrl = it },
                            )
                            PaperField(
                                label = tr("API KEY", "API 密钥"),
                                value = apiKey,
                                // Declared to the platform, so no keyboard over
                                // this page learns a credential and nothing of
                                // it appears on the screen.
                                isSecret = true,
                                onValueChange = { apiKey = it },
                            )
                        }
                        refusal?.let { kind ->
                            Text(
                                text = AiChapterCopy.refusal(kind, tr),
                                style = TypviaType.Caption.style(tr.language),
                                color = Paper.attention,
                                modifier = Modifier.padding(top = SettingsMetrics.rowGap),
                            )
                        }
                        PaperVerb(
                            isPrimary = true,
                            label = if (isWorking) {
                                tr("Saving", "正在存")
                            } else {
                                tr("Save", "存下")
                            },
                            enabled = !isWorking,
                        ) {
                            onChoose(candidate, model, baseUrl, apiKey.ifEmpty { null })
                            // Out of this screen the instant it is handed over.
                            apiKey = ""
                        }
                    }
                }
            }

            Text(
                // Standing, whatever is chosen above it: a property of the
                // product rather than of the current selection.
                text = tr(
                    "A snippet from the vault is never sent to any model, whichever of these is chosen.",
                    "保险库里的片段永远不会被送去任何模型,无论这里选的是哪一项。",
                ),
                style = TypviaType.Mono.style(tr.language),
                color = Paper.ink3,
                modifier = Modifier.padding(
                    top = Tokens.Space.group,
                    bottom = Tokens.Space.section,
                ),
            )
        }
    }
}

/** One engine, with the sentence that says what choosing it means. */
@Composable
private fun Choice(
    engine: AiEngine,
    isChosen: Boolean,
    isInEffect: Boolean,
    choose: () -> Unit,
) {
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
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = SettingsMetrics.rowGap)
            .background(plate, RoundedCornerShape(Tokens.Radius.control))
            .clickable(onClick = choose)
            .semantics(mergeDescendants = true) { selected = isInEffect }
            .padding(SettingsMetrics.numberGap),
    ) {
        Text(
            text = AiChapterCopy.title(engine, tr) +
                // Which one the product is actually using, said in words
                // rather than by the plate alone: the plate also marks the one
                // being looked at, and a reader must be able to tell those two
                // apart without comparing shades of paper.
                if (isInEffect) tr(" · in use", " · 在用") else "",
            style = TypviaType.SectionTitle.style(tr.language),
            color = Paper.ink,
        )
        Text(
            text = AiChapterCopy.detail(engine, tr),
            style = TypviaType.Caption.style(tr.language),
            color = Paper.ink2,
            modifier = Modifier.padding(top = SettingsMetrics.fieldInnerGap),
        )
    }
}

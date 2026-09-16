// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import dev.typvia.mobile.ffi.TypviaStore
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Room
import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.TypviaType
import uniffi.typvia_mobile_ffi.AiAction
import uniffi.typvia_mobile_ffi.CoreException

/**
 * What an AI action is allowed to be asked, from a snippet's page.
 *
 * The rule that matters most here is the one this file does not enforce: a
 * snippet from the vault is refused behind the bridge, before a prompt exists.
 * This passes the flag through and never decides it — a screen deciding what
 * counts as sensitive would be a second opinion on the one question that must
 * have exactly one.
 */
sealed interface AiStripPhase {
    /** Nothing asked. The actions are offered as words. */
    data object Idle : AiStripPhase

    /** A round trip is out. */
    data object Working : AiStripPhase

    /**
     * What came back, waiting to be taken or dropped.
     *
     * [refusal] is set when taking it failed. The text stays on screen with
     * it: what came back is the expensive part, and dropping it to report that
     * it could not be saved would make one failure into two.
     */
    data class Produced(
        val text: String,
        val maskedKinds: List<String>,
        val refusal: AiRefusal? = null,
    ) : AiStripPhase

    /** It did not happen, and the strip says which kind of "did not". */
    data class Refused(val kind: AiRefusal) : AiStripPhase

    /**
     * No engine is configured. Not a failure — a thing not set up — so the
     * strip says where to set it up rather than reporting an error.
     */
    data object NoEngine : AiStripPhase
}

/**
 * Why something did not happen. Kinds, never the engine's own words.
 *
 * The kinds are the core's, one for one. Collapsing two of them into one
 * sentence is how a screen ends up saying something false: the first run of
 * this strip reported a provider that rejected the request as "that action is
 * no longer here", because a rejection and a missing record had been given the
 * same kind here.
 *
 * **What the core does not distinguish, this cannot either**: a refusal by the
 * egress gate — the one refusal in this product most worth naming — arrives as
 * [NoResult], the same kind as a missing API key and a provider saying no. So
 * [NoResult]'s sentence claims nothing about whether anything left the device.
 */
enum class AiRefusal {
    /** Something in what was typed is not usable. */
    NotUsable,

    /** Nothing came back, and nothing here changed. */
    NoResult,

    /** The engine could not be reached. Offline-shaped, not a fault. */
    Unreachable,

    /** It needs something it has not been given. */
    NotPermitted,

    /** The record addressed is not there. */
    Missing,

    /** Storage failed. Never quoted: those messages can carry paths and SQL. */
    Storage,
    ;

    companion object {
        fun of(error: Throwable): AiRefusal = when (error) {
            is CoreException.Validation -> NotUsable
            is CoreException.Conflict -> NoResult
            is CoreException.Unavailable -> Unreachable
            is CoreException.PermissionDenied -> NotPermitted
            is CoreException.RuleBlocked -> NotPermitted
            is CoreException.NotFound -> Missing
            else -> Storage
        }
    }
}

/**
 * One saved action, as this page lists it.
 *
 * [inputSource] is a contract rather than a hint: an action states where its
 * text may come from, and running one with text from anywhere else is refused
 * behind the bridge. Offering an action this page cannot keep the contract for
 * would be worse than not offering it.
 */
data class AiActionRow(val id: String, val name: String, val inputSource: String) {
    val canRunHere: Boolean get() = inputSource == AiKeeper.SOURCE

    companion object {
        fun of(action: AiAction): AiActionRow =
            AiActionRow(id = action.id, name = action.name, inputSource = action.inputSource)
    }
}

/** The calls this page makes, and the only place it makes them. */
object AiKeeper {
    /** The source a snippet's page supplies. */
    const val SOURCE = "snippet"

    /**
     * The actions this page may offer. Null when the list could not be read —
     * which is not the same as "there are none", and must not be drawn as if
     * it were.
     */
    suspend fun actions(store: TypviaStore): List<AiActionRow>? =
        runCatching {
            store.perform { core -> core.aiActionList().map(AiActionRow::of) }
        }.getOrNull()?.filter { it.canRunHere }

    /** Whether an engine is configured at all. */
    suspend fun hasEngine(store: TypviaStore): Boolean =
        runCatching { store.perform { it.aiProviderList().isNotEmpty() } }.getOrDefault(false)

    /**
     * Runs one action over a snippet's body.
     *
     * [isSensitive] is the snippet's own state, passed through rather than
     * asserted to be false: this page is only reachable for ordinary snippets,
     * and telling the gate "not sensitive" would be this side vouching for
     * something it is not the authority on.
     */
    suspend fun run(
        store: TypviaStore,
        action: AiActionRow,
        text: String,
        isSensitive: Boolean,
    ): Result<AiStripPhase.Produced> = runCatching {
        store.perform { core ->
            val result = core.aiActionRun(action.id, text, SOURCE, isSensitive)
            AiStripPhase.Produced(result.output, result.maskedKinds)
        }
    }
}

/** The sentences the strip says. */
object AiStripCopy {
    fun refusal(kind: AiRefusal, tr: Translator): String = when (kind) {
        AiRefusal.NotUsable -> tr(
            "The engine would not take that as it is.",
            "引擎不接受这样的内容。",
        )
        // True whichever of the several things the core folds into this kind
        // happened — the gate refusing to send, no key, the provider saying
        // no. It says what the reader can check and claims nothing about what
        // did or did not leave, because at this layer that is not known.
        AiRefusal.NoResult -> tr(
            "Nothing came back, and nothing here changed.",
            "没有拿到结果,这里也什么都没变。",
        )
        // The core's own reading: an unreachable engine is a state to show,
        // not a fault to report. The product goes on working without it.
        AiRefusal.Unreachable -> tr(
            "The engine could not be reached. Everything here is as it was.",
            "没能连上引擎。这里的东西一如原样。",
        )
        AiRefusal.NotPermitted -> tr(
            "This needs something it has not been given.",
            "这件事需要一项它还没有的东西。",
        )
        AiRefusal.Missing -> tr(
            "That action is no longer here.",
            "这个动作已经不在了。",
        )
        AiRefusal.Storage -> tr(
            "It did not go through, and nothing here changed.",
            "没走通,这里也什么都没变。",
        )
    }

    /** What was hidden on the way out, by kind. Empty when nothing was. */
    fun masked(kinds: List<String>, tr: Translator): String? {
        if (kinds.isEmpty()) return null
        return tr(
            "Masked before it left: ${kinds.joinToString(", ")}",
            "出门前被遮掉的:${kinds.joinToString("、")}",
        )
    }
}

/**
 * The AI strip on a snippet's page.
 *
 * It never acts on its own: what comes back is a proposal the reader takes or
 * drops. The strip is only ever composed after the page has established that
 * this snippet is not a secret — a secret's page returns before reaching here,
 * so the vault rule holds by where this stands rather than by a flag somebody
 * has to remember to pass.
 */
@Composable
fun AiStrip(
    phase: AiStripPhase,
    actions: List<AiActionRow>,
    onRun: (AiActionRow) -> Unit,
    onTake: (String) -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
    Column(modifier = modifier.padding(top = DetailMetrics.verbTop)) {
        when (phase) {
            AiStripPhase.Idle -> {
                if (actions.isEmpty()) return@Column
                Text(
                    text = tr("AI ACTIONS", "AI 动作"),
                    style = TypviaType.MonoLabel.style(tr.language),
                    color = Paper.ink3,
                )
                Row(
                    modifier = Modifier.padding(top = DetailMetrics.metaTop),
                    horizontalArrangement = Arrangement.spacedBy(DetailMetrics.verbGap),
                ) {
                    for (action in actions) {
                        Text(
                            text = action.name,
                            style = TypviaType.SectionTitle.style(tr.language),
                            color = Room.Ai.accent,
                            modifier = Modifier.clickable { onRun(action) },
                        )
                    }
                }
            }
            AiStripPhase.Working -> Text(
                // No spinner in this product: the sentence is the wait.
                text = tr("Asking the model", "正在问模型"),
                style = TypviaType.Mono.style(tr.language),
                color = Paper.ink3,
            )
            is AiStripPhase.Produced -> {
                Text(
                    text = phase.text,
                    style = TypviaType.Mono.style(tr.language),
                    color = Paper.ink,
                )
                // Named, not counted: if something was hidden on the way out
                // the reader is told what kind of thing it was.
                AiStripCopy.masked(phase.maskedKinds, tr)?.let { line ->
                    Text(
                        text = line,
                        style = TypviaType.Mono.style(tr.language),
                        color = Paper.attention,
                        modifier = Modifier.padding(top = DetailMetrics.metaTop),
                    )
                }
                phase.refusal?.let { kind ->
                    Text(
                        text = AiStripCopy.refusal(kind, tr),
                        style = TypviaType.BodyS.style(tr.language),
                        color = Paper.ink2,
                        modifier = Modifier.padding(top = DetailMetrics.metaTop),
                    )
                }
                Row(
                    modifier = Modifier.padding(top = DetailMetrics.verbTop),
                    horizontalArrangement = Arrangement.spacedBy(DetailMetrics.verbGap),
                ) {
                    Text(
                        text = tr("Use this", "用这段"),
                        style = TypviaType.SectionTitle.style(tr.language),
                        color = Paper.ink,
                        modifier = Modifier.clickable { onTake(phase.text) },
                    )
                    Text(
                        text = tr("Leave it", "不用"),
                        style = TypviaType.BodyS.style(tr.language),
                        color = Paper.ink2,
                        modifier = Modifier.clickable(onClick = onDismiss),
                    )
                }
            }
            is AiStripPhase.Refused -> Note(AiStripCopy.refusal(phase.kind, tr), onDismiss)
            AiStripPhase.NoEngine -> Note(
                tr(
                    "No engine is set up yet — settings chapter 03 chooses one.",
                    "还没有选引擎——设置第 03 章里选一个。",
                ),
                onDismiss,
            )
        }
    }
}

/** One sentence and the word that closes it. */
@Composable
private fun Note(sentence: String, onDismiss: () -> Unit) {
    val tr = LocalTranslator.current
    Text(
        text = sentence,
        style = TypviaType.BodyS.style(tr.language),
        color = Paper.ink2,
    )
    Text(
        text = tr("Close", "知道了"),
        style = TypviaType.BodyS.style(tr.language),
        color = Paper.ink3,
        modifier = Modifier
            .padding(top = DetailMetrics.metaTop)
            .clickable(onClick = onDismiss),
    )
}

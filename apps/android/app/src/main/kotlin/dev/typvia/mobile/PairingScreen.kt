// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import dev.typvia.mobile.ffi.TypviaStore
import dev.typvia.mobile.ui.LocalReduceMotion
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Room
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.TypviaType
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Joining an account, as a page.
 *
 * A page and not a sheet: this product has one modal and it belongs to the
 * vault. The reader says where the library lives, holds up a code, and then
 * compares the check string with the other screen — and only that last step
 * installs anything, which is why it is the one the page cannot skip past.
 *
 * @param onLeave the caller drops the session as it closes this page. A code
 *   left running on a screen nobody is looking at is a code somebody else
 *   could still be answering.
 * @param onJoined this device is in: the account, the devices and the library
 *   all have to be read again.
 */
@Composable
fun PairingScreen(
    store: TypviaStore,
    onLeave: () -> Unit,
    onJoined: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
    val scope = rememberCoroutineScope()
    val reduceMotion = LocalReduceMotion.current

    var ask by remember { mutableStateOf(PairingAsk()) }
    var step by remember { mutableStateOf<PairingStep?>(null) }
    var offered by remember { mutableStateOf<PairingCode?>(null) }
    var reveal by remember { mutableStateOf<SasReveal?>(null) }
    var refusal by remember { mutableStateOf<PairingRefusal?>(null) }
    var isWorking by remember { mutableStateOf(false) }
    /** Nil while unread, and nil again when the read failed — see [PairingAsk.canBegin]. */
    var canHoldAKey by remember { mutableStateOf<Boolean?>(null) }
    /** Typed only where it can be needed, and never kept: the master key is
     * re-wrapped under this device's own password. */
    var masterPassword by remember { mutableStateOf("") }

    LaunchedEffect(store) { canHoldAKey = PairingKeeper.canHoldAKey(store) }

    val offering = step as? PairingStep.Offering
    val comparing = step as? PairingStep.Comparing

    // Waiting for the other device. Every two seconds, and every failed ask is
    // simply "not yet" — the other screen is being read by a person.
    LaunchedEffect(offering) {
        if (offering == null) return@LaunchedEffect
        while (true) {
            delay(PairingKeeper.POLL_SECONDS * 1000)
            val claim = PairingKeeper.poll(store) ?: continue
            step = PairingStep.Comparing(claim.sas, claim.rootFingerprint)
            return@LaunchedEffect
        }
    }

    // The window counts down only where a window was promised. Nothing counts
    // down to zero on its own here — the code is stated, never swapped out
    // mid-sentence while the reader is reading it aloud.
    LaunchedEffect(offering) {
        if (offering == null || offered?.secondsLeft == null) return@LaunchedEffect
        while (true) {
            delay(1000)
            val left = offered?.secondsLeft ?: return@LaunchedEffect
            if (left <= 0) return@LaunchedEffect
            offered = offered?.copy(secondsLeft = left - 1)
        }
    }

    // The characters land one at a time, because the reader is being asked to
    // compare rather than to glance. A reader who asked the system for less
    // motion gets the whole string at once — the comparison is the point, not
    // the way it arrives.
    LaunchedEffect(comparing, reduceMotion) {
        val sas = comparing?.sas ?: return@LaunchedEffect
        // The separators are not characters anybody compares, so they are not
        // beats either: the count comes from the reveal, not from the string.
        val count = SasReveal(sas, 0).characters.size
        if (reduceMotion) {
            reveal = SasReveal(sas, count)
            return@LaunchedEffect
        }
        reveal = SasReveal(sas, 0)
        val beat = SasReveal.stepMillis(count)
        for (shown in 1..count) {
            delay(beat)
            reveal = SasReveal(sas, shown)
        }
    }

    fun drop() {
        scope.launch { PairingKeeper.cancel(store) }
        offered = null
        reveal = null
        masterPassword = ""
        ask = ask.copy(password = "")
        step = PairingStep.Dropped
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Paper.base)
            .verticalScroll(rememberScrollState())
            // Fields are typed into on this page, and a verb under the
            // keyboard is a verb the reader cannot reach.
            .imePadding()
            .padding(horizontal = Tokens.Space.screenPadding),
    ) {
        Text(
            text = tr("Back", "返回"),
            style = TypviaType.BodyS.style(tr.language),
            color = Paper.ink2,
            modifier = Modifier
                .padding(top = 14.dp, bottom = SettingsMetrics.headGap)
                .clickable(onClick = onLeave),
        )

        when (val current = step) {
            null -> JoinTarget(
                ask = ask,
                canHoldAKey = canHoldAKey,
                isWorking = isWorking,
                onAsk = { ask = it },
                onBegin = {
                    scope.launch {
                        isWorking = true
                        refusal = null
                        PairingKeeper.begin(store, ask).fold(
                            onSuccess = { started ->
                                // The typed secret is not kept once it has been
                                // handed over.
                                ask = ask.copy(password = "")
                                offered = PairingCode(started.code, started.expiresInSeconds)
                                step = PairingStep.Offering(started.code)
                            },
                            onFailure = { refusal = PairingRefusal.of(it) },
                        )
                        isWorking = false
                    }
                },
            )

            is PairingStep.Offering -> Offer(
                code = offered ?: PairingCode(current.code, null),
            )

            is PairingStep.Comparing -> Compare(
                reveal = reveal ?: SasReveal(current.sas, 0),
                fingerprint = current.fingerprint,
                masterPassword = masterPassword,
                isWorking = isWorking,
                onMasterPassword = { masterPassword = it },
                onSame = {
                    scope.launch {
                        isWorking = true
                        refusal = null
                        PairingKeeper.finalize(store, masterPassword).fold(
                            onSuccess = {
                                masterPassword = ""
                                step = PairingStep.Joined
                                onJoined()
                            },
                            onFailure = { refusal = PairingRefusal.of(it) },
                        )
                        isWorking = false
                    }
                },
                onNotSame = ::drop,
            )

            PairingStep.Joined -> Joined(onDone = onLeave)

            PairingStep.Dropped -> Dropped(
                onTryAgain = {
                    refusal = null
                    step = null
                    offered = null
                },
                onLeaveIt = onLeave,
            )
        }

        refusal?.let { said ->
            Text(
                text = PairingCopy.sentence(said, tr),
                style = TypviaType.Caption.style(tr.language),
                color = Paper.attention,
                modifier = Modifier.padding(top = PairingMetrics.gap),
            )
        }
        Box(modifier = Modifier.height(Tokens.Space.section))
    }
}

/**
 * Where the library this device is joining actually lives.
 *
 * The delivery's pairing frames start at the code, but a code cannot be asked
 * for without knowing which account it is for. So this step is built in the
 * settings chapter's own language: words rather than controls, one question at
 * a time, and not one icon.
 */
@Composable
private fun JoinTarget(
    ask: PairingAsk,
    canHoldAKey: Boolean?,
    isWorking: Boolean,
    onAsk: (PairingAsk) -> Unit,
    onBegin: () -> Unit,
) {
    val tr = LocalTranslator.current
    Text(
        text = tr("Join an account", "加入一个账户"),
        style = TypviaType.Title2.style(tr.language),
        color = Paper.ink,
    )
    Text(
        text = tr(
            "The key passes between the two devices. The server only carries the ciphertext.",
            "钥匙在两台设备之间直接交换。服务器只搬密文。",
        ),
        style = TypviaType.BodyS.style(tr.language),
        color = Paper.ink2,
        modifier = Modifier.padding(top = PairingMetrics.innerGap),
    )
    if (canHoldAKey == false) {
        Text(
            // Said here rather than after a filled-in form: this device cannot
            // keep a sync key, so no address would have worked.
            text = tr(
                "This device cannot keep a sync key, so it cannot join an account yet. Everything else works, and your snippets stay on it.",
                "这台设备存不住同步钥匙,所以暂时加入不了账户。其余功能照常,片段也都还在。",
            ),
            style = TypviaType.BodyS.style(tr.language),
            color = Paper.ink2,
            modifier = Modifier.padding(top = PairingMetrics.gap),
        )
    }
    Row(modifier = Modifier.padding(top = PairingMetrics.gap)) {
        Choice(
            label = tr("My own server", "自建服务器"),
            isChosen = ask.target == PairingTarget.Server,
            onTap = { onAsk(ask.copy(target = PairingTarget.Server)) },
        )
        Box(modifier = Modifier.width(PairingMetrics.gap))
        Choice(
            label = tr("WebDAV", "WebDAV"),
            isChosen = ask.target == PairingTarget.Webdav,
            onTap = { onAsk(ask.copy(target = PairingTarget.Webdav)) },
        )
    }
    Text(
        text = when (ask.target) {
            PairingTarget.Server -> tr(
                "The address and the account id are both on the other device's sync page.",
                "服务器地址与账户 ID 都在另一台设备的同步页上。",
            )
            PairingTarget.Webdav -> tr(
                "Point at the same folder the other device uses — the account is read from the storage itself.",
                "指向另一台设备用的同一个目录——账户信息直接从存储里读。",
            )
        },
        style = TypviaType.Caption.style(tr.language),
        color = Paper.ink3,
        modifier = Modifier.padding(top = PairingMetrics.innerGap),
    )
    Field(
        label = tr("ADDRESS", "地址"),
        value = ask.address,
        onValueChange = { onAsk(ask.copy(address = it)) },
    )
    when (ask.target) {
        PairingTarget.Server -> Field(
            label = tr("ACCOUNT ID", "账户 ID"),
            value = ask.accountId,
            onValueChange = { onAsk(ask.copy(accountId = it)) },
        )
        PairingTarget.Webdav -> {
            Field(
                label = tr("USERNAME", "用户名"),
                value = ask.username,
                onValueChange = { onAsk(ask.copy(username = it)) },
            )
            Field(
                label = tr("PASSWORD", "密码"),
                value = ask.password,
                isSecret = true,
                onValueChange = { onAsk(ask.copy(password = it)) },
            )
        }
    }
    PaperVerb(
        label = if (isWorking) tr("Asking…", "正在请求…") else tr("Show the code", "出示配对码"),
        enabled = ask.canBegin(canHoldAKey) && !isWorking,
        onTap = onBegin,
    )
}

/** Step one: this device's code, held out for the other one to read. */
@Composable
private fun Offer(code: PairingCode) {
    val tr = LocalTranslator.current
    StepCount(step = 1)
    Text(
        text = tr("Hand this one to the other device", "把这台交给另一台"),
        style = TypviaType.Title2.style(tr.language),
        color = Paper.ink,
        modifier = Modifier.padding(top = PairingMetrics.innerGap),
    )
    Text(
        text = tr(
            "Open Typvia on the other device and give it this code. The key passes between the two of them and never goes through the server.",
            "在另一台设备上打开 Typvia,把这个码给它。钥匙在两台设备之间直接交换,不经过服务器。",
        ),
        style = TypviaType.BodyS.style(tr.language),
        color = Paper.ink2,
        modifier = Modifier.padding(top = PairingMetrics.innerGap),
    )
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = PairingMetrics.gap)
            .background(Paper.carrier, RoundedCornerShape(Tokens.Radius.card))
            .padding(PairingMetrics.slipPadding),
    ) {
        Text(
            text = PairingCode.grouped(code.code),
            style = TypviaType.Mono.style(tr.language),
            color = Paper.ink,
        )
    }
    // A code nobody promised a window for says nothing about validity, rather
    // than announcing an expiry no one undertook.
    PairingCopy.validity(code, tr)?.let { said ->
        Text(
            text = said,
            style = TypviaType.Mono.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = PairingMetrics.innerGap),
        )
    }
    Text(
        // Kept on screen for as long as it is true.
        text = tr(
            "Waiting for the other device. Nothing has been installed here yet.",
            "正在等另一台设备。这台上还什么都没装。",
        ),
        style = TypviaType.Mono.style(tr.language),
        color = Paper.ink3,
        modifier = Modifier.padding(top = PairingMetrics.gap),
    )
}

/**
 * Step two: the characters both devices arrived at.
 *
 * If the two sides read the same, nothing sat in the middle. This is the only
 * step that installs anything, and the only one the reader answers.
 */
@Composable
private fun Compare(
    reveal: SasReveal,
    fingerprint: String,
    masterPassword: String,
    isWorking: Boolean,
    onMasterPassword: (String) -> Unit,
    onSame: () -> Unit,
    onNotSame: () -> Unit,
) {
    val tr = LocalTranslator.current
    StepCount(step = 2)
    Text(
        text = tr("Is it the same on both?", "两边是同一串吗?"),
        style = TypviaType.Title2.style(tr.language),
        color = Paper.ink,
        modifier = Modifier.padding(top = PairingMetrics.innerGap),
    )
    Text(
        text = tr(
            "Each of these characters was worked out half by one device and half by the other. If both sides read the same, nothing sat in between.",
            "这串字符由两台设备各算一半得出。只要两边一样,中间就没有第三者。",
        ),
        style = TypviaType.BodyS.style(tr.language),
        color = Paper.ink2,
        modifier = Modifier.padding(top = PairingMetrics.innerGap),
    )
    Column(
        verticalArrangement = Arrangement.spacedBy(PairingMetrics.sasLineGap),
        modifier = Modifier
            .padding(top = PairingMetrics.gap)
            // One thing to read rather than twenty: read out character by
            // character, the check is a stream of letters with no grouping.
            .semantics(mergeDescendants = true) { contentDescription = reveal.spoken },
    ) {
        for (row in reveal.rows) {
            Row(horizontalArrangement = Arrangement.spacedBy(PairingMetrics.sasGap)) {
                for (group in row) {
                    Text(
                        text = group.text,
                        style = TypviaType.Code.style(tr.language),
                        color = Paper.ink,
                    )
                }
            }
        }
    }
    Box(
        modifier = Modifier
            .fillMaxWidth(reveal.progress)
            .height(PairingMetrics.sasRule)
            .background(Room.Vault.accent),
    )
    Text(
        // Not a device name: this side of the exchange has never been told one.
        // What it does hold is the account's fingerprint, which is the thing
        // the other screen can also show.
        text = tr("Account fingerprint $fingerprint", "账户指纹 $fingerprint"),
        style = TypviaType.Mono.style(tr.language),
        color = Paper.ink3,
        modifier = Modifier.padding(top = PairingMetrics.gap),
    )
    Text(
        text = tr(
            "Master password for this device's vault — only if the account has one.",
            "这台设备保险库的主密码——只有账户里有保险库时才需要。",
        ),
        style = TypviaType.Caption.style(tr.language),
        color = Paper.ink3,
        modifier = Modifier.padding(top = PairingMetrics.gap),
    )
    Field(
        label = tr("MASTER PASSWORD", "主密码"),
        value = masterPassword,
        isSecret = true,
        onValueChange = onMasterPassword,
    )
    PaperVerb(
        label = if (isWorking) tr("Joining…", "正在加入…") else tr("Same — carry on", "一样,继续"),
        // Answerable only once the whole string is on screen: half a string is
        // not something a reader can have compared.
        enabled = reveal.isComplete && !isWorking,
        onTap = onSame,
    )
    PaperVerb(
        label = tr("Not the same", "不一样"),
        enabled = !isWorking,
        onTap = onNotSame,
    )
    Text(
        text = tr(
            "Choosing \"not the same\" drops the connection and asks for a new code. The snippets you already have are untouched.",
            "选「不一样」会立刻断开并重出码,已有的片段不受影响。",
        ),
        style = TypviaType.Caption.style(tr.language),
        color = Paper.ink3,
        modifier = Modifier.padding(top = PairingMetrics.gap),
    )
}

@Composable
private fun Joined(onDone: () -> Unit) {
    val tr = LocalTranslator.current
    Text(
        text = tr("This device is in.", "这台设备加入了。"),
        style = TypviaType.Title2.style(tr.language),
        color = Paper.ink,
    )
    Text(
        text = tr(
            "Your snippets will arrive on their own. Secrets stay locked until you open the vault here.",
            "片段会自己到。保险库里的东西在这台设备上解锁之前保持锁定。",
        ),
        style = TypviaType.BodyS.style(tr.language),
        color = Paper.ink2,
        modifier = Modifier.padding(top = PairingMetrics.innerGap),
    )
    PaperVerb(label = tr("Done", "好"), onTap = onDone)
}

/** Stopped — by the reader, or by a round that failed. What is said first is
 * what is still true. */
@Composable
private fun Dropped(onTryAgain: () -> Unit, onLeaveIt: () -> Unit) {
    val tr = LocalTranslator.current
    Text(
        text = PairingCopy.nothingInstalled(tr),
        style = TypviaType.Title2.style(tr.language),
        color = Paper.ink,
    )
    Text(
        text = tr(
            "The session is dropped and the code is dead. Your snippets on both devices are untouched.",
            "会话已经断开,那个码作废了。两台设备上已有的片段都没有被动过。",
        ),
        style = TypviaType.BodyS.style(tr.language),
        color = Paper.ink2,
        modifier = Modifier.padding(top = PairingMetrics.innerGap),
    )
    PaperVerb(label = tr("Try again", "再来一次"), onTap = onTryAgain)
    PaperVerb(label = tr("Leave it", "先不了"), onTap = onLeaveIt)
}

/** Which of the two numbered steps this is. */
@Composable
private fun StepCount(step: Int) {
    val tr = LocalTranslator.current
    Text(
        text = "$step / ${PairingStep.TOTAL}",
        style = TypviaType.MonoLabel.style(tr.language),
        color = Paper.ink3,
    )
}

/** One of the two places this library could be. Chosen by weight, never by a
 * filled pill. */
@Composable
private fun Choice(label: String, isChosen: Boolean, onTap: () -> Unit) {
    val tr = LocalTranslator.current
    Text(
        text = label,
        style = TypviaType.BodyS.style(tr.language),
        color = if (isChosen) Paper.ink else Paper.ink3,
        modifier = Modifier.clickable(onClick = onTap),
    )
}

/**
 * One line to type on: a caps mono label, the text, and a hairline under it.
 *
 * @param isSecret masks the glyphs **and tells the platform what the field is**
 *   — a masked field that never declared itself still ends up in a third-party
 *   keyboard's suggestion strip, and this product does not get to promise what
 *   it never told the system.
 */
@Composable
private fun Field(
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
        modifier = Modifier.padding(top = PairingMetrics.gap),
    )
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        singleLine = true,
        visualTransformation = if (isSecret) PasswordVisualTransformation() else VisualTransformation.None,
        keyboardOptions = KeyboardOptions(
            keyboardType = if (isSecret) KeyboardType.Password else KeyboardType.Uri,
            autoCorrectEnabled = false,
        ),
        textStyle = TypviaType.Mono.style(tr.language).copy(color = Paper.ink),
        cursorBrush = SolidColor(Room.Settings.accent),
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = PairingMetrics.innerGap),
    )
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = PairingMetrics.innerGap)
            .height(Tokens.Line.hairlineWidth)
            .background(Paper.rule),
    )
}

private object PairingMetrics {
    val gap = 18.dp
    val innerGap = 8.dp
    val slipPadding = 14.dp
    val sasGap = 10.dp
    val sasLineGap = 10.dp
    val sasRule = 2.dp
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.TypviaType

/**
 * What the keyboard chapter says, as values.
 *
 * Every sentence here is about what this keyboard does and does not do, and
 * every one of them is checkable against the file it reads. None of it is
 * reassurance: "we respect your privacy" is a sentence that appears on
 * products that do not.
 */
object KeyboardChapterCopy {
    /**
     * Whether this product's keyboard is switched on, said in one sentence.
     *
     * Null is not "off". The phone was asked and did not answer, and printing
     * "not on" would be this page answering a question it never got an answer
     * to — then sending the reader to a settings screen where they find it was
     * on all along.
     */
    fun headline(isOn: Boolean?, tr: Translator): String = when (isOn) {
        true -> tr(
            "It is switched on in this phone's keyboard list.",
            "它已经在这台手机的输入法清单里打开了。",
        )
        false -> tr(
            "It is not switched on yet, so it does not come up when you type.",
            "还没有在系统里打开,所以你打字的时候它不会出现。",
        )
        null -> tr(
            "This phone did not say which keyboards are switched on.",
            "这台手机没有说哪些输入法是开着的。",
        )
    }

    /** The verb that leaves for the system's own list. Always offered. */
    fun openSystemList(tr: Translator): String = tr(
        "Open the phone's keyboard list",
        "打开手机的输入法清单",
    )

    /**
     * Offered only where the keyboard is known to be on: switching to a
     * keyboard that is not in the list opens a chooser it is not in, which
     * reads as the product being broken rather than as it not being on.
     */
    fun switchToIt(tr: Translator): String = tr("Switch to Typvia", "切换到 Typvia")
}

/**
 * The keyboard chapter: whether it is on, what it may read, and the two rules
 * that decide what a reader sees on it.
 *
 * The delivery draws the keyboard itself at length but gives this chapter no
 * frame of its own, so it is set in the language the other chapters use:
 * the numeral as the page's header, caps mono labels with their statement
 * under them, and verbs as words rather than buttons.
 */
@Composable
fun KeyboardChapterScreen(
    isOn: Boolean?,
    onClose: () -> Unit,
    onOpenSystemList: () -> Unit,
    onSwitchToIt: () -> Unit,
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
        Text(text = "02", style = TypviaType.Title1.style(tr.language), color = Paper.ink3)
        Text(
            text = tr("Keyboard", "键盘"),
            style = TypviaType.Title2.style(tr.language),
            color = Paper.ink,
            modifier = Modifier.padding(top = SettingsMetrics.rowGap),
        )
        Text(
            text = KeyboardChapterCopy.headline(isOn, tr),
            style = TypviaType.BodyS.style(tr.language),
            color = Paper.ink2,
            modifier = Modifier.padding(top = SettingsMetrics.rowGap),
        )
        // Switching on happens in the system's own screen — no app can do it —
        // so what this page owes the reader is the shortest way there.
        PaperVerb(
            KeyboardChapterCopy.openSystemList(tr),
            isPrimary = isOn != true,
            onTap = onOpenSystemList,
        )
        if (isOn == true) {
            PaperVerb(KeyboardChapterCopy.switchToIt(tr), onTap = onSwitchToIt)
        }

        Line(
            label = tr("WHAT IT MAY READ", "它能读到什么"),
            value = tr(
                "A file this app writes for it — not your library.",
                "一份这个应用专门写给它的文件,不是你的库本身。",
            ),
            detail = tr(
                "It runs on its own and never opens the database. A vault snippet is in that file as an identifier and nothing else: the keyboard cannot read its name, let alone what is in it.",
                "它自己单独跑,从不打开数据库。保险库里的片段在那份文件里只有一个编号:键盘读不到它的名字,更读不到里面的东西。",
            ),
        )
        Line(
            label = tr("TRIGGER MARKS", "触发符"),
            value = tr(
                "A trigger belongs to the snippet, not to this page.",
                "触发词属于片段自己,不在这一页设置。",
            ),
            detail = tr(
                "Type one on the keyboard's own search line and the bench narrows as you type — it looks at titles, triggers and the text itself. A vault snippet never matches: there is nothing there to match against.",
                "在键盘自己的搜索行里打一个,台面就边打边收窄——它看的是标题、触发词和正文本身。保险库的片段永远不会被搜到:那里根本没有可以拿来比对的东西。",
            ),
        )
        Line(
            label = tr("ORDER", "排序"),
            value = tr(
                "Most recently used first, then the order your library keeps.",
                "最近用过的在最前面,其余按你库里的顺序。",
            ),
            detail = tr(
                "Favourites are a filter on the keyboard rather than a place in the queue — starring something does not push it in front of what you actually reach for.",
                "收藏在键盘上是一个筛子,不是排在前面的位置——给一枚片段加星,不会把它推到你真正常用的那些前面。",
            ),
        )
        Line(
            label = tr("LANGUAGE", "语言"),
            value = tr(
                "It follows the language you chose in Look & language.",
                "它跟着你在「外观与语言」里选的那种语言走。",
            ),
            detail = tr(
                "It reads that choice each time it comes up, so a change reaches it the next time you type — without restarting anything.",
                "它每次出现时读一次那个选择,所以你改了以后,下一次打字它就跟上了——不需要重启任何东西。",
            ),
        )
        Text(
            text = tr(
                "It does not go online, and it does not keep what you type.",
                "它不联网,也不留下你打的字。",
            ),
            style = TypviaType.Caption.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(
                top = Tokens.Space.group,
                bottom = Tokens.Space.section,
            ),
        )
    }
}

/** One stated fact: a caps mono label, the statement, then why it matters. */
@Composable
private fun Line(label: String, value: String, detail: String) {
    val tr = LocalTranslator.current
    Text(
        text = label,
        style = TypviaType.MonoLabel.style(tr.language),
        color = Paper.ink3,
        modifier = Modifier.padding(top = Tokens.Space.group),
    )
    Text(
        text = value,
        style = TypviaType.Body.style(tr.language),
        color = Paper.ink,
        modifier = Modifier.padding(top = SettingsMetrics.rowGap),
    )
    Text(
        text = detail,
        style = TypviaType.Caption.style(tr.language),
        color = Paper.ink2,
        modifier = Modifier.padding(top = 6.dp),
    )
}

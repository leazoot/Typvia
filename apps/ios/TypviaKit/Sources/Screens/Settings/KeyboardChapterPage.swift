// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// Every word the keyboard chapter says, in one place.
///
/// Each sentence is about what this keyboard does and does not do, and each
/// one is checkable against the document it reads. None of it is reassurance:
/// "we respect your privacy" is a sentence that appears on products that do
/// not.
enum KeyboardChapterCopy {
    /// Whether the keyboard has been added, said in one sentence.
    ///
    /// The negative is written as *not confirmed* rather than as *off*: this
    /// device answers with a list it does not always give us, so "it is not
    /// there" and "we could not see it" arrive as the same answer. Printing
    /// "off" over the second one sends a reader to add a keyboard they added
    /// already.
    static func headline(_ install: KeyboardInstall, _ tr: Translator) -> String {
        switch install {
        case .ready:
            return tr(
                "It is in this phone's keyboard list.",
                "它在这台手机的键盘清单里。"
            )
        case .notAdded:
            return tr(
                "This phone does not report it as added yet. Adding it is done in the system's own settings — no app can do it for you.",
                "这台手机还没有报告说它被添加了。添加这件事在系统自己的设置里做——没有哪个应用能替你做。"
            )
        }
    }

    /// The contents page's right-hand column.
    static func contentsValue(_ install: KeyboardInstall, _ tr: Translator) -> String {
        switch install {
        case .ready: return tr("added", "已添加")
        case .notAdded: return tr("not confirmed", "未确认")
        }
    }

    static func openSystemSettings(_ tr: Translator) -> String {
        tr("Open this app's system settings", "打开这个应用的系统设置")
    }
}

/// The keyboard chapter: whether it has been added, what it may read, and the
/// two rules that decide what a reader sees on it.
///
/// The delivery draws the keyboard itself at length but gives this chapter no
/// frame of its own, so it is set in the language the other chapters use: caps
/// mono labels with their statement under them, and verbs as words.
struct KeyboardChapter: View {
    @Environment(\.tr) private var tr

    let install: KeyboardInstall
    /// Absent where there is nowhere to send the reader. Present, it leaves
    /// for the one screen that can switch a keyboard on.
    var openSystemSettings: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(KeyboardChapterCopy.headline(install, tr))
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)
            if let openSystemSettings {
                StickVerb(
                    title: KeyboardChapterCopy.openSystemSettings(tr),
                    isPrimary: install == .notAdded,
                    action: openSystemSettings
                )
                .padding(.top, SettingsMetrics.rowGap)
            }

            stated(
                label: tr("FULL ACCESS", "完全访问权限"),
                value: tr(
                    "This keyboard does not ask for it.",
                    "这个键盘不申请它。"
                ),
                // Said this way round because a reader who goes looking will
                // not find the switch: iOS draws it only for keyboards that
                // request it, so an absent switch here is the setting working,
                // not a setting missing.
                detail: tr(
                    "Because it does not ask, iOS shows no such switch for Typvia — if you go looking for it, that is why it is not there. A keyboard without full access cannot reach the network at all, and this one has nothing to send anywhere.",
                    "正因为不申请,系统设置里 Typvia 这一项根本没有这个开关——你去找它找不到,原因就在这里。没有完全访问权限的键盘根本连不上网络,而这个键盘也没有什么要往外送。"
                )
            )
            stated(
                label: tr("WHAT IT MAY READ", "它能读到什么"),
                value: tr(
                    "A document this app writes for it — not your library.",
                    "一份这个应用专门写给它的文件,不是你的库本身。"
                ),
                detail: tr(
                    "It runs on its own and never opens the database. A vault snippet is in that document as an identifier and nothing else: the keyboard cannot read its name, let alone what is in it.",
                    "它自己单独跑,从不打开数据库。保险库里的片段在那份文件里只有一个编号:键盘读不到它的名字,更读不到里面的东西。"
                )
            )
            stated(
                label: tr("TRIGGER MARKS", "触发符"),
                value: tr(
                    "A trigger belongs to the snippet, not to this page.",
                    "触发词属于片段自己,不在这一页设置。"
                ),
                detail: tr(
                    "Type one on the keyboard's own search line and the bench narrows as you type — it looks at titles, triggers and the text itself. A vault snippet never matches: there is nothing there to match against.",
                    "在键盘自己的搜索行里打一个,台面就边打边收窄——它看的是标题、触发词和正文本身。保险库的片段永远不会被搜到:那里根本没有可以拿来比对的东西。"
                )
            )
            stated(
                label: tr("ORDER", "排序"),
                value: tr(
                    "Most recently used first, then the order your library keeps.",
                    "最近用过的在最前面,其余按你库里的顺序。"
                ),
                detail: tr(
                    "Favourites are a filter on the keyboard rather than a place in the queue — starring something does not push it in front of what you actually reach for.",
                    "收藏在键盘上是一个筛子,不是排在前面的位置——给一枚片段加星,不会把它推到你真正常用的那些前面。"
                )
            )
            stated(
                label: tr("LANGUAGE", "语言"),
                value: tr(
                    "It follows the language you chose in Appearance & language.",
                    "它跟着你在「外观与语言」里选的那种语言走。"
                ),
                detail: tr(
                    "It reads that choice each time it comes up, so a change reaches it the next time you type.",
                    "它每次出现时读一次那个选择,所以你改了以后,下一次打字它就跟上了。"
                )
            )
            Text(
                tr(
                    "It types into the field you are in, character by character. Nothing goes through the clipboard, and it keeps no record of what you type.",
                    "它把文字一个字一个字打进你所在的输入框。什么都不经过剪贴板,也不留下你打的字。"
                )
            )
            .typviaType(.mono)
            .foregroundStyle(Paper.ink3)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, SettingsMetrics.paragraphGap)
        }
    }

    /// One stated fact: a caps mono label with the statement under it.
    ///
    /// Each of these used to trail a paragraph explaining why it is that way.
    /// Five statements are a page a reader scans; five statements each
    /// dragging a paragraph is a wall — and the reasoning is written down in
    /// the repository, which is where a reader is not standing when they open
    /// a settings chapter. `detail` is kept as a parameter so the reasons stay
    /// beside the facts they belong to in the source.
    private func stated(label: String, value: String, detail: String) -> some View {
        _ = detail
        return VStack(alignment: .leading, spacing: 0) {
            PaperLabel(label)
                .padding(.top, SettingsMetrics.lineGap)
            Text(value)
                .typviaType(.body)
                .foregroundStyle(Paper.ink)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, SettingsMetrics.lineInnerGap)
        }
    }
}

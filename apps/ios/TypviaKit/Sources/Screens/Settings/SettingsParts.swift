// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation
import SwiftUI

/// One line of the contents page: the number in the margin, the title, one
/// line of what the chapter is about, and its current value in mono.
struct ChapterLine: View {
    @Environment(\.tr) private var tr

    let chapter: SettingsChapter
    let value: String
    /// Every chapter opens now. This was optional while some rows were print
    /// only, and the number was set a shade lighter for those — a row that
    /// looks like a way in and does nothing is worse than one that plainly is
    /// not one.
    let open: () -> Void

    var body: some View {
        let line = HStack(alignment: .top, spacing: SettingsMetrics.lineGap) {
            Text(chapter.number)
                .typviaType(.heading)
                .foregroundStyle(Paper.ink2)
                .frame(width: SettingsMetrics.numberWidth, alignment: .leading)
            VStack(alignment: .leading, spacing: SettingsMetrics.lineInnerGap) {
                Text(SettingsCopy.title(chapter, tr: tr))
                    .typviaType(.sectionTitle)
                    .foregroundStyle(Paper.ink)
                Text(SettingsCopy.summary(chapter, tr: tr))
                    .typviaType(.caption)
                    .foregroundStyle(Paper.ink3)
                    .fixedSize(horizontal: false, vertical: true)
            }
            // The middle column takes the slack; the value column keeps its
            // own width. Without this the summary claims its ideal width and
            // pushes the value — the column a reader is actually scanning —
            // off the edge of the sheet.
            .frame(maxWidth: .infinity, alignment: .leading)
            Text(value)
                .typviaType(.mono)
                .foregroundStyle(Paper.ink2)
                .layoutPriority(1)
                .multilineTextAlignment(.trailing)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .frame(minHeight: Tokens.Hit.minimum)
        // Without this the row responds only where there is ink. A reader
        // tapping the empty half of the line — which on this page is most of
        // it — got nothing, and a contents page whose rows sometimes work is
        // read as a contents page that is broken.
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)

        Button(action: open) { line }.buttonStyle(.plain)
    }
}

/// A chapter, opened: the number becomes the page's header and stays in the
/// margin where it was.
struct ChapterPage: View {
    @Environment(\.tr) private var tr

    let chapter: SettingsChapter
    @ObservedObject var model: SettingsModel
    let actions: SettingsActions
    @ObservedObject var preferences: UiPreferences
    let close: () -> Void
    @State private var confirmingDeletion: String?

    var body: some View {
        ScrollView(showsIndicators: false) {
            VStack(alignment: .leading, spacing: 0) {
                Button(action: close) {
                    Text(tr("Back", "返回"))
                        .typviaType(.bodyS)
                        .foregroundStyle(Paper.ink2)
                        .frame(minHeight: Tokens.Hit.minimum)
                }
                .buttonStyle(.plain)
                Text(chapter.number)
                    .typviaType(.display)
                    .foregroundStyle(Paper.ink2)
                    .padding(.top, SettingsMetrics.headerNumberTop)
                Text(SettingsCopy.title(chapter, tr: tr))
                    .typviaType(.heading)
                    .foregroundStyle(Paper.ink)
                    .padding(.top, SettingsMetrics.headerGap)
                body(for: chapter)
                    .padding(.top, SettingsMetrics.paragraphGap)
            }
            .padding(.horizontal, Tokens.Space.screenPadding)
            .padding(.top, SettingsMetrics.top)
            .padding(.bottom, Tokens.Space.section)
        }
        .swipeToGoBack(close)
    }

    @ViewBuilder
    private func body(for chapter: SettingsChapter) -> some View {
        switch chapter {
        case .sync:
            SyncChapter(model: model, pair: actions.pair)
        case .ai:
            AiChapter(engine: model.engine, refusal: model.refusal) { engine, name, url, key in
                Task { await model.choose(engine, model: name, baseUrl: url, apiKey: key) }
            }
        case .data:
            VStack(alignment: .leading, spacing: 0) {
                DataChapter(stats: model.stats)
                DataInAndOut(model: model)
                TrashSection(
                    rows: model.trash,
                    now: Int64(Date().timeIntervalSince1970 * 1000),
                    restore: { id in Task { await model.putBack(id) } },
                    deleteForever: { id in Task { await model.deleteForever(id) } },
                    confirming: $confirmingDeletion
                )
                .padding(.top, SettingsMetrics.dangerGap)
            }
            .task { await model.loadTrash() }
        case .appearance:
            AppearanceChapter(preferences: preferences)
        case .keyboard:
            KeyboardChapter(
                install: KeyboardInstall.current(),
                openSystemSettings: actions.openSystemSettings
            )
        case .vault:
            VaultChapter(
                facts: model.vault,
                said: model.vaultSaid,
                refusal: model.vaultRefusal,
                keep: { password in Task { await model.useFaceId(password: password, tr) } },
                remove: { Task { await model.stopUsingFaceId(tr) } },
                forget: { model.clearVaultReport() }
            )
        }
    }
}

/// The sync chapter. One sentence explains the encryption without a shield and
/// without the words "military grade"; then the devices this key recognises.
struct SyncChapter: View {
    @Environment(\.tr) private var tr

    @ObservedObject var model: SettingsModel
    let pair: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // The claim in one clause. What it rests on is below, behind a
            // word — a chapter that opens with its own reasoning makes the
            // reader read an argument to find a state.
            Text(tr("Encrypted before it leaves this device.", "离开这台设备之前就已加密。"))
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)
            if let trouble = model.trouble {
                SyncTroubleNote(
                    trouble: trouble,
                    isRetrying: model.isRetrying,
                    retry: { Task { await model.retrySync() } }
                )
                .padding(.top, SettingsMetrics.paragraphGap)
            }
            Text(tr("Devices this key recognises", "这把钥匙认得的设备"))
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.ink3)
                .padding(.top, SettingsMetrics.paragraphGap)
            if model.devices.isEmpty {
                Text(
                    tr(
                        "None yet — this device is the only copy.",
                        "还没有——这台设备是唯一的副本。"
                    )
                )
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink2)
                .padding(.top, SettingsMetrics.rowGap)
            } else {
                VStack(alignment: .leading, spacing: SettingsMetrics.rowGap) {
                    ForEach(model.devices) { device in
                        DeviceLine(device: device)
                    }
                }
                .padding(.top, SettingsMetrics.rowGap)
            }
            if let pair {
                StickVerb(title: tr("Start pairing", "开始配对"), isPrimary: true, action: pair)
                    .padding(.top, SettingsMetrics.actionGap)
            }
        }
    }

    private func note(_ sentence: String) -> some View {
        Text(sentence)
            .typviaType(.caption)
            .foregroundStyle(Paper.ink2)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, SettingsMetrics.lineInnerGap)
    }
}

/// A device row: its name, what it is doing, and — for the ones that are not
/// this device — a way to remove it.
struct DeviceLine: View {
    @Environment(\.tr) private var tr

    let device: DeviceRow

    var body: some View {
        HStack(alignment: .top, spacing: SettingsMetrics.lineGap) {
            VStack(alignment: .leading, spacing: SettingsMetrics.lineInnerGap) {
                Text(device.name)
                    .typviaType(.sectionTitle)
                    .foregroundStyle(Paper.ink)
                Text(detail)
                    .typviaType(.mono)
                    .foregroundStyle(Paper.ink3)
            }
            Spacer(minLength: 0)
            Text(stateWord)
                .typviaType(.mono)
                .foregroundStyle(isWaiting ? Paper.attention : Paper.ink2)
        }
        .accessibilityElement(children: .combine)
    }

    private var isWaiting: Bool {
        if case .waiting = device.state { return true }
        return false
    }

    /// The state word is mono, in the same family as the numbers, and clay
    /// rather than red when something is held up.
    private var stateWord: String {
        switch device.state {
        case .inUse: tr("in use", "在用")
        case .waiting: tr("waiting", "待发")
        case .lastSeen: tr("paired", "已配对")
        }
    }

    private var detail: String {
        switch device.state {
        case .inUse:
            return tr("this one", "这一台")
        case let .waiting(count):
            return tr("this one · \(count) to send", "这一台 · 有 \(count) 条待发")
        case let .lastSeen(stamp):
            guard let stamp else { return tr("removed", "已移除") }
            return tr(
                "last heard from \(RelativeStamp.text(for: stamp, language: tr.language))",
                "最后一次收到 · \(RelativeStamp.text(for: stamp, language: tr.language))"
            )
        }
    }
}

/// Sync has been failing. Written the same way round as the home screen's
/// card: everything is still here, then what happened, then one action.
struct SyncTroubleNote: View {
    @Environment(\.tr) private var tr

    let trouble: SyncTrouble
    let isRetrying: Bool
    let retry: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: SettingsMetrics.lineInnerGap) {
            Text(tr("SYNC HELD UP", "同步受阻"))
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.attention)
            Text(
                trouble.total == 1
                    ? tr(
                        "Your one snippet is here, on this device.",
                        "你那一枚片段还在,就在这台设备上。"
                    )
                    : tr(
                        "All \(trouble.total) snippets are here, on this device.",
                        "\(trouble.total) 枚片段一枚没少,全在这台设备上。"
                    )
            )
            .typviaType(.sectionTitle)
            .foregroundStyle(Paper.ink)
            .fixedSize(horizontal: false, vertical: true)
            Text(problem)
                .typviaType(.caption)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)
            StickVerb(
                title: isRetrying ? tr("Trying", "正在重试") : tr("Retry now", "现在重试"),
                action: retry
            )
        }
    }

    private var problem: String {
        let since = trouble.lastSyncAt
            .map { RelativeStamp.text(for: $0, language: tr.language) }
            ?? tr("a while ago", "有一段时间了")
        // The verb agrees as well as the noun, which is why this is a branch
        // and not the shared noun helper.
        if trouble.queued == 1 {
            return tr(
                "The server has not answered since \(since). 1 change is queued and goes out on its own when the network is back — there is nothing for you to do.",
                "服务器从 \(since) 起没再应答。有 1 条改动排在队里等着,网络回来会自己送出去,不需要你做什么。"
            )
        }
        return tr(
            "The server has not answered since \(since). \(trouble.queued) changes are queued and go out on their own when the network is back — there is nothing for you to do.",
            "服务器从 \(since) 起没再应答。\(trouble.queued) 条改动排在队里等着,网络回来会自己送出去,不需要你做什么。"
        )
    }
}

/// The AI chapter, which is the one place a room changes colour: clay, because
/// the room follows the content rather than the navigation.
struct AiChapter: View {
    @Environment(\.tr) private var tr
    @State private var apiKey = ""
    @State private var modelName = ""
    @State private var baseUrl = ""

    let engine: AiEngine
    let refusal: Refusal?
    /// Absent while nothing can be changed. Present, it is how a choice takes
    /// effect — by writing the providers, which is what everything else reads.
    var choose: ((AiEngine, String, String, String?) -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // One line, and it is the line that decides the choice below it.
            Text(tr("Where your text goes when an AI action runs.", "跑 AI 动作时,你的文字去哪里。"))
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)
            VStack(alignment: .leading, spacing: SettingsMetrics.optionGap) {
                option(.onThisDevice)
                option(.ownKey)
                if engine != .off, choose != nil {
                    engineFields
                }
                option(.off)
            }
            .padding(.top, SettingsMetrics.paragraphGap)
            // Standing, whatever is chosen above it. It is a property of the
            // product, not of the current selection — and it is the one claim
            // on this page worth printing outright rather than tucking away.
            Text(
                tr(
                    "A snippet from the vault is never sent to any model, whichever of these is chosen.",
                    "保险库里的片段永远不会被送去任何模型,无论这里选的是哪一项。"
                )
            )
            .typviaType(.mono)
            .foregroundStyle(Paper.ink3)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, SettingsMetrics.paragraphGap)
        }
    }

    /// The chosen one gets a slip of paper under it; the others are only
    /// words. The weight of the choice is shown by material, not by a control.
    /// The key is typed here and goes straight to the secure store behind the
    /// bridge; it is never held by this screen beyond the keystroke, never
    /// logged, and never synced.
    /// The model name, and — for an account — the key. The delivery's frame
    /// asks for neither, but a provider without a model is not a provider and
    /// the engine refuses one; asking is more honest than inventing a model
    /// name on the reader's behalf. The key goes straight to the secure store
    /// behind the bridge: never held here beyond the keystroke, never logged,
    /// never synced.
    private var engineFields: some View {
        VStack(alignment: .leading, spacing: SettingsMetrics.lineInnerGap) {
            TextField(
                "",
                text: $modelName,
                prompt: Text(tr("Model name", "模型名")).foregroundColor(Paper.ink3)
            )
            .typviaType(.mono)
            .foregroundStyle(Paper.ink)
            .tint(Room.ai.accent)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            if engine == .ownKey {
                TextField(
                    "",
                    text: $baseUrl,
                    prompt: Text(tr("Endpoint", "接口地址")).foregroundColor(Paper.ink3)
                )
                .typviaType(.mono)
                .foregroundStyle(Paper.ink)
                .tint(Room.ai.accent)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                SecureField(
                    "",
                    text: $apiKey,
                    prompt: Text(tr("API key", "API 密钥")).foregroundColor(Paper.ink3)
                )
                .typviaType(.mono)
                .foregroundStyle(Paper.ink)
                .tint(Room.ai.accent)
            }
            if let refusal {
                Text(sentence(for: refusal))
                    .typviaType(.caption)
                    .foregroundStyle(Paper.attention)
                    .fixedSize(horizontal: false, vertical: true)
            }
            StickVerb(title: tr("Save", "存下"), isPrimary: true) {
                choose?(engine, modelName, baseUrl, apiKey.isEmpty ? nil : apiKey)
                apiKey = ""
            }
        }
        .padding(.leading, SettingsMetrics.optionPadding)
    }

    private func sentence(for refusal: Refusal) -> String {
        SettingsCopy.engineRefusal(refusal, tr: tr)
    }

    private func option(_ candidate: AiEngine) -> some View {
        let isChosen = candidate == engine
        return VStack(alignment: .leading, spacing: SettingsMetrics.lineInnerGap) {
            Text(SettingsCopy.engineTitle(candidate, tr: tr))
                .typviaType(.sectionTitle)
                .foregroundStyle(Paper.ink)
            // The difference between the three, in as many words as the
            // difference takes. What each one is *like* to live with is worth
            // saying and is not what a reader is deciding here, so it waits
            // below with the rest.
            Text(SettingsCopy.engineDistinction(candidate, tr: tr))
                .typviaType(.caption)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(isChosen ? SettingsMetrics.optionPadding : 0)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            if isChosen {
                RoundedRectangle(cornerRadius: Tokens.Radius.card, style: .continuous)
                    .fill(Paper.carrier)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isChosen ? [.isSelected] : [])
        .contentShape(Rectangle())
        .onTapGesture { choose?(candidate, modelName, baseUrl, nil) }
    }
}

/// The data chapter, whose headline figure is set large enough to be a layout
/// element in its own right.
struct DataChapter: View {
    @Environment(\.tr) private var tr

    let stats: DataStats

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(String(stats.total))
                .typviaType(.display)
                .foregroundStyle(Paper.ink)
            // The figure above is set on its own, so the noun under it agrees
            // with a number this line does not print.
            Text(
                tr(stats.total == 1 ? "snippet" : "snippets", "枚片段")
                    + " · "
                    + tr.counted(stats.sorts, "kind", "kinds", "\(stats.sorts) 类")
            )
            .typviaType(.mono)
            .foregroundStyle(Paper.ink3)
            .padding(.top, SettingsMetrics.figureGap)
            Hairline()
                .padding(.vertical, SettingsMetrics.paragraphGap)
            // The one destructive thing in the room. No red fill: a clay mono
            // label and a sentence that says what actually happens stops a
            // reader more surely than a red button does.
            Text(tr("CANNOT BE UNDONE", "不可撤销"))
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.attention)
                .padding(.top, SettingsMetrics.dangerGap)
            Text(
                tr(
                    "Delete every snippet on this device",
                    "删除这台设备上的全部片段"
                )
            )
            .typviaType(.sectionTitle)
            .foregroundStyle(Paper.ink)
            .padding(.top, SettingsMetrics.lineInnerGap)
            // Not folded away with the rest. What a destructive act does and
            // does not reach is the one thing a reader must not have to ask
            // for — the whole page may be quieter, and this line stays.
            Text(
                tr(
                    "Copies on other devices are not affected. An export is required first.",
                    "其它设备上的副本不受影响。删除前会让你先导出一次。"
                )
            )
            .typviaType(.caption)
            .foregroundStyle(Paper.ink2)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, SettingsMetrics.lineInnerGap)
        }
    }
}

/// Appearance and language. Two choices, each a short row of words — the only
/// place in the product where a setting is chosen rather than reported.
struct AppearanceChapter: View {
    @Environment(\.tr) private var tr
    @ObservedObject var preferences: UiPreferences

    var body: some View {
        VStack(alignment: .leading, spacing: SettingsMetrics.switchRowGap) {
            group(tr("Language", "界面语言")) {
                ForEach(UiPreference.Language.allCases, id: \.rawValue) { option in
                    choice(name(option), isChosen: preferences.language == option) {
                        preferences.language = option
                    }
                }
            }
            group(tr("Appearance", "外观")) {
                ForEach(UiPreference.Appearance.allCases, id: \.rawValue) { option in
                    choice(name(option), isChosen: preferences.appearance == option) {
                        preferences.appearance = option
                    }
                }
            }
            // Said outright rather than tucked away: a reader has no way to
            // know the other surfaces hear this at all, and each of them is a
            // separate process that nothing tells.
            Text(
                tr(
                    "The keyboard, the widgets and the share sheet follow this too.",
                    "键盘、小组件与分享面板也跟着这里走。"
                )
            )
            .typviaType(.mono)
            .foregroundStyle(Paper.ink3)
            .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func group(_ label: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: SettingsMetrics.lineInnerGap) {
            Text(label)
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.ink3)
            HStack(spacing: SettingsMetrics.actionGap) {
                content()
                Spacer(minLength: 0)
            }
        }
    }

    /// The chosen one is ink; the others are meta. No switch, no pill — the
    /// weight of a choice is shown the way this product shows it elsewhere.
    private func choice(_ title: String, isChosen: Bool, choose: @escaping () -> Void) -> some View {
        Button(action: choose) {
            Text(title)
                .typviaType(isChosen ? .sectionTitle : .bodyS)
                .foregroundStyle(isChosen ? Paper.ink : Paper.ink3)
                .frame(minHeight: Tokens.Hit.minimum)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(isChosen ? [.isSelected] : [])
    }

    private func name(_ language: UiPreference.Language) -> String {
        switch language {
        case .system: tr("System", "跟随系统")
        case .en: "English"
        case .zh: "中文"
        }
    }

    private func name(_ appearance: UiPreference.Appearance) -> String {
        switch appearance {
        case .system: tr("System", "跟随系统")
        case .light: tr("Light", "浅色")
        case .dark: tr("Dark", "深色")
        }
    }
}

/// The recycle bin, inside the data chapter.
///
/// The delivery has no frame for it, so it is set in this room's own language:
/// rows like the library's, one clay label for the half that cannot be undone,
/// and the confirmation inline rather than in a dialog.
struct TrashSection: View {
    @Environment(\.tr) private var tr

    let rows: [TrashRow]
    let now: Int64
    let restore: ((String) -> Void)?
    let deleteForever: ((String) -> Void)?
    @Binding var confirming: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(tr("Recycle bin", "回收站"))
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.ink3)
            if rows.isEmpty {
                Text(tr("Nothing is waiting here.", "这里没有东西在等着。"))
                    .typviaType(.bodyS)
                    .foregroundStyle(Paper.ink2)
                    .padding(.top, SettingsMetrics.rowGap)
            } else {
                VStack(alignment: .leading, spacing: SettingsMetrics.rowGap) {
                    ForEach(rows) { row in
                        line(row)
                    }
                }
                .padding(.top, SettingsMetrics.rowGap)
            }
        }
    }

    private func line(_ row: TrashRow) -> some View {
        VStack(alignment: .leading, spacing: SettingsMetrics.lineInnerGap) {
            HStack(alignment: .top, spacing: SettingsMetrics.lineGap) {
                if let sort = row.sort {
                    TypeSortMark(sort, size: .compact, accessibilityLabel: sort.name(tr))
                }
                VStack(alignment: .leading, spacing: SettingsMetrics.lineInnerGap) {
                    Text(row.title)
                        .typviaType(.sectionTitle)
                        .foregroundStyle(Paper.ink)
                    Text(deadline(row))
                        .typviaType(.mono)
                        .foregroundStyle(Paper.ink3)
                }
                Spacer(minLength: 0)
                if let restore {
                    StickVerb(title: tr("Put back", "放回去")) { restore(row.id) }
                }
            }
            if confirming == row.id {
                // Inline, and it says the number: a confirmation that does not
                // name what it is about is a confirmation people click through.
                HStack(spacing: SettingsMetrics.lineGap) {
                    Text(tr("Delete \"\(row.title)\" for good?", "永久删除「\(row.title)」?"))
                        .typviaType(.caption)
                        .foregroundStyle(Paper.attention)
                        .fixedSize(horizontal: false, vertical: true)
                    if let deleteForever {
                        StickVerb(title: tr("Delete", "删除")) {
                            deleteForever(row.id)
                            confirming = nil
                        }
                    }
                    StickVerb(title: tr("Keep", "留着")) { confirming = nil }
                }
            } else {
                StickVerb(title: tr("Delete for good", "永久删除")) { confirming = row.id }
            }
        }
        .accessibilityElement(children: .contain)
    }

    /// What is useful is the deadline, not how long ago it went.
    private func deadline(_ row: TrashRow) -> String {
        guard let left = TrashWindow.daysLeft(deletedAt: row.deletedAt, now: now) else {
            return tr("kept for now", "暂时保留")
        }
        return left == 0
            ? tr("cleared on the next sweep", "下次清理时移除")
            : tr.counted(left, "day left", "days left", "还剩 \(left) 天")
    }
}

/// A caps mono section label. Shared by the chapters that state facts in
/// labelled lines rather than in prose.
struct PaperLabel: View {
    private let text: String

    init(_ text: String) { self.text = text }

    var body: some View {
        Text(text)
            .typviaType(.monoLabel)
            .foregroundStyle(Paper.ink3)
            .padding(.top, SettingsMetrics.rowGap)
    }
}

/// One line to type on. A secret declares itself to the platform, so no
/// keyboard over this page learns a passphrase or a master password.
struct PaperField: View {
    let label: String
    @Binding var text: String
    var isSecret = false

    var body: some View {
        VStack(alignment: .leading, spacing: SettingsMetrics.lineInnerGap) {
            // Not the shared label: that one carries the gap that separates
            // one stated fact from the next, and inside a field the label sits
            // against its own line.
            Text(label)
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.ink3)
            if isSecret {
                SecureField("", text: $text)
                    .typviaType(.mono)
                    .foregroundStyle(Paper.ink)
                    .tint(Room.settings.accent)
                    .textContentType(.password)
            } else {
                TextField("", text: $text)
                    .typviaType(.mono)
                    .foregroundStyle(Paper.ink)
                    .tint(Room.settings.accent)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }
            Hairline()
        }
        .padding(.top, SettingsMetrics.rowGap)
    }
}

/// Every word this room says, in one place.
enum SettingsCopy {
    static func title(_ chapter: SettingsChapter, tr: Translator) -> String {
        switch chapter {
        case .sync: tr("Sync", "同步")
        case .keyboard: tr("Keyboard", "键盘")
        case .ai: tr("AI engine", "AI 引擎")
        case .vault: tr("Vault", "保险库")
        case .appearance: tr("Appearance & language", "外观与语言")
        case .data: tr("Data", "数据")
        }
    }

    /// The one line under each title.
    ///
    /// It names what the chapter's page holds, and nothing else: a contents
    /// page is where a reader decides whether it is worth opening, so a verb
    /// printed here that the page does not have sends them looking for it.
    static func summary(_ chapter: SettingsChapter, tr: Translator) -> String {
        switch chapter {
        case .sync: tr("End-to-end encrypted · devices", "端到端加密 · 设备")
        case .keyboard: tr("Trigger characters · order · full access", "触发符 · 排序 · 完全访问权限")
        case .ai: tr("Which machine rewriting runs on", "改写与生成在哪台机器上跑")
        case .vault: tr("How it opens · when it shuts · screenshots", "解锁方式 · 自动回锁 · 截屏保护")
        case .appearance: tr("Theme · type size · language", "主题 · 字号 · 界面语言")
        case .data: tr("Export · import · the recycle bin", "导出 · 导入 · 回收站")
        }
    }

    /// Why a change to the engine did not take.
    ///
    /// Every sentence holds one line: a change that did not take leaves the
    /// engine exactly as it was. None of them may read as "off" over an engine
    /// that is still configured.
    ///
    /// The kinds are stated one to one rather than falling through to the
    /// shared sentences. The shared one for a broken rule is "something here is
    /// already taken", and on this page that kind is a provider the engine
    /// refused or a key the credential rules would not accept — neither of
    /// which is anything being taken. Listing every kind also means a new one
    /// cannot arrive here without a sentence of its own.
    static func engineRefusal(_ refusal: Refusal, tr: Translator) -> String {
        switch refusal {
        case .invalid:
            tr("That address or model is not one the engine can use.", "这个地址或模型引擎用不了。")
        case .clash, .storage:
            tr("That did not take. The engine is as it was.", "没改成。引擎还是原来的样子。")
        case .offline:
            tr(
                "The engine could not be reached. The engine is as it was.",
                "没能连上引擎。引擎还是原来的样子。"
            )
        case .notPermitted, .ruleBlocked:
            tr(
                "This needs something it has not been given. The engine is as it was.",
                "这件事需要一项它还没有的东西。引擎还是原来的样子。"
            )
        case .missing:
            tr("That engine is no longer here.", "这个引擎已经不在了。")
        }
    }

    static func engineTitle(_ engine: AiEngine, tr: Translator) -> String {
        switch engine {
        case .onThisDevice: tr("On this device", "就在这台设备上")
        case .ownKey: tr("With my own API key", "用我自己的 API 密钥")
        case .off: tr("Turn AI actions off", "关掉 AI 动作")
        }
    }

    /// What separates one engine from the other two, in one clause.
    ///
    /// This is the whole of what a reader is deciding at the moment they scan
    /// these three; the fuller line each one used to carry says what living
    /// with it is like, which matters afterwards rather than during.
    static func engineDistinction(_ engine: AiEngine, tr: Translator) -> String {
        switch engine {
        case .onThisDevice: tr("The text does not leave.", "文字不出门。")
        case .ownKey: tr("Through your own account.", "走你自己的账号。")
        case .off: tr("Nothing goes to any model.", "什么都不交给模型。")
        }
    }

    static func engineDetail(_ engine: AiEngine, tr: Translator) -> String {
        switch engine {
        case .onThisDevice:
            tr(
                "The text does not leave. Speed depends on the machine; long text is slower.",
                "文字不出门。速度取决于机器,长文会慢一些。"
            )
        case .ownKey:
            tr(
                "Through your own account. The key is kept in the vault and never syncs.",
                "走你自己的账号。密钥存进保险库,不随同步上云。"
            )
        case .off:
            // The delivery gives this as "the AI chapter disappears; the other
            // seven kinds carry on", which is not true of this build: the
            // library lists all eight kinds whichever engine is chosen. What
            // does happen is stated instead.
            tr(
                "No text from this device goes to any model. The key is removed with it.",
                "这台设备上的文字不会再交给任何模型。密钥也一并删掉。"
            )
        }
    }
}

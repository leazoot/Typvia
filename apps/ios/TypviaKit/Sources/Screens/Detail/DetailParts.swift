// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// The title and the body: the part of the screen the snippet actually is.
///
/// It is its own view so that every one of its states — read, edit, secret,
/// template, brand new — can be put on a page and looked at.
struct DetailBody: View {
    @Environment(\.tr) private var tr
    @ObservedObject var model: DetailModel
    /// Handed down from the screen, which is the level that can scroll.
    var isComposing: FocusState<Bool>.Binding?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if model.snippet == nil, model.mode == .reading {
                // Opened by a link to something that is no longer here. An
                // empty page would leave the reader wondering whether it
                // failed to load or the snippet is gone.
                Text(tr("This snippet is no longer here.", "这枚片段已经不在了。"))
                    .typviaType(.title2)
                    .foregroundStyle(Paper.ink)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, DetailMetrics.titleTop)
                Text(
                    tr(
                        "It may have been deleted on this device or on another one.",
                        "它可能在这台设备上被删了,也可能是在另一台。"
                    )
                )
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, DetailMetrics.titleGap)
            } else {
                titleBlock
                bodyBlock
                    .padding(.top, DetailMetrics.bodyTop)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        // A secret's page carries its own ground. The screen sets the same
        // one, but a room that trusts its container is a room that renders as
        // invisible type the first time it is put somewhere else — which is
        // exactly what happened to the library's shut chapter.
        .background(model.isSecret ? VaultRoom.base : Color.clear)
    }

    @ViewBuilder
    private var titleBlock: some View {
        if model.mode == .reading {
            VStack(alignment: .leading, spacing: DetailMetrics.titleGap) {
                Text(model.snippet?.title ?? "")
                    .typviaType(.title2)
                    .foregroundStyle(model.isSecret ? VaultRoom.ink : Paper.ink)
                if let trigger = model.snippet?.trigger {
                    Text(trigger)
                        .typviaType(.mono)
                        .foregroundStyle(Room.home.accent)
                }
                metaLine
            }
            .padding(.top, DetailMetrics.titleTop)
        } else {
            TextField(
                "",
                text: $model.draft.title,
                prompt: Text(tr("Give it a name", "起个名字")).foregroundColor(Paper.ink3)
            )
            .typviaType(.title2)
            .foregroundStyle(Paper.ink)
            .tint(Room.home.accent)
            .padding(.top, DetailMetrics.titleTop)
        }
    }

    @ViewBuilder
    private var metaLine: some View {
        if let snippet = model.snippet {
            let sort = TypeSort(coreType: snippet.snippetType)
            let kind = sort.map { $0.name(tr) } ?? ""
            let used = model.isSecret ? tr("Vault", "保险库") : usageWord(snippet.usageCount)
            Text("\(kind) · \(used)")
                .typviaType(.mono)
                .foregroundStyle(model.isSecret ? VaultRoom.ink3 : Paper.ink3)
        }
    }

    /// How often it has been used. English wants "once" where the count is one,
    /// and the phrase carries a verb as well as a noun, so it branches here
    /// rather than through the shared noun helper.
    private func usageWord(_ count: UInt64) -> String {
        count == 1 ? tr("used once", "1 次") : tr("used \(count) times", "\(count) 次")
    }

    @ViewBuilder
    private var bodyBlock: some View {
        if model.isSecret {
            SecretBody(model: model)
        } else if model.mode == .reading, let template = model.templateBody {
            TemplateBodyView(model: model, template: template)
        } else if model.mode == .reading {
            Text(model.snippet?.body ?? "")
                .typviaType(.mono)
                .foregroundStyle(Paper.ink)
                .frame(maxWidth: .infinity, alignment: .leading)
                .textSelection(.enabled)
                .background(material)
        } else {
            ComposerBody(model: model, material: material, isFocused: isComposing)
        }
    }

    @ViewBuilder
    private var material: some View {
        if ChapterStyle.of(model.draft.effectiveSort).material == .grid {
            GridPaper()
        }
    }
}

/// The composing stick: every tool this screen has, along the bottom.
///
/// Collapsed it is three verbs and a handle. Opened it carries the kind, the
/// trigger and the folder — none of which go in a top bar, because a thumb
/// does not reach a top bar.
struct ComposingStick: View {
    @Environment(\.tr) private var tr
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ObservedObject var model: DetailModel
    @Binding var isOpen: Bool
    @State private var isNamingFolder = false
    @State private var folderName = ""

    let actions: DetailActions
    let onSave: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            handle
            if model.mode == .reading {
                readingVerbs
            } else {
                editorTools
            }
        }
        .padding(.horizontal, Tokens.Space.screenPadding)
        .padding(.top, DetailMetrics.stickTop)
        .padding(.bottom, DetailMetrics.stickTop)
        .frame(maxWidth: .infinity, alignment: .leading)
        // The stick goes to the vault's material while the vault is what it is
        // asking about — the same break in material the room itself makes.
        .background(model.isSecret || model.secretGate != nil ? VaultRoom.carrier : Paper.carrier)
        .animation(Beat.transition.enter(reduceMotion: reduceMotion), value: isOpen)
        .animation(Beat.transition.enter(reduceMotion: reduceMotion), value: model.mode)
    }

    @ViewBuilder
    private var handle: some View {
        if model.mode == .reading, !model.isSecret {
            Capsule()
                .fill(Paper.rule)
                .frame(width: DetailMetrics.stickHandleWidth, height: DetailMetrics.stickHandleHeight)
                .frame(maxWidth: .infinity, alignment: .trailing)
                .padding(.bottom, DetailMetrics.stickFieldGap)
                .accessibilityHidden(true)
        }
    }

    /// Three verbs, no icons. A secret gets none of them: taking it out is the
    /// only thing that happens on that screen, and that button lives with the
    /// body it uncovers.
    @ViewBuilder
    private var readingVerbs: some View {
        if !model.isSecret, model.snippet != nil {
            HStack(spacing: DetailMetrics.stickGap) {
                if let copy = actions.copy, let body = model.snippet?.body {
                    StickVerb(title: tr("Copy", "复制")) {
                        copy(body)
                        Task { await model.recordUse() }
                    }
                }
                if let insert = actions.insert, let body = model.snippet?.body {
                    StickVerb(title: tr("Insert", "插入")) {
                        insert(body)
                        Task { await model.recordUse() }
                    }
                }
                StickVerb(title: tr("Edit", "编辑")) {
                    model.edit()
                    isOpen = true
                }
                // Moving something already saved into the vault. Not a kind to
                // choose — it re-encrypts the body and drops the plaintext
                // history this snippet has been keeping — so it is a verb of
                // its own rather than a mark in the kind row.
                StickVerb(title: tr("Put in the vault", "放进保险库")) {
                    Task { await model.promoteToSecret() }
                }
                if !model.fields.isEmpty, let insert = actions.insert {
                    // A template goes in filled. "Copy the original" is beside
                    // it for the times a reader wants the blanks themselves.
                    StickVerb(title: tr("Fill and insert", "填空并插入"), isPrimary: true) {
                        Task {
                            guard let filled = await model.filled() else { return }
                            insert(filled)
                        }
                    }
                }
                Spacer(minLength: 0)
                savedNote
                    // The gap the delivery gives it. Without one, a row with
                    // enough verbs on it pushes the note straight up against
                    // the last of them.
                    .padding(.leading, DetailMetrics.savedNoteGap)
            }
            if !model.isSecret {
                AiStrip(model: model)
            }
        }
    }

    /// "Saved · just now", typed out and then let go of. It is not a toast and
    /// it does not cover anything.
    @ViewBuilder
    private var savedNote: some View {
        if model.savedAt != nil {
            Text(tr("Saved · just now", "已存 · 刚刚"))
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
                .transition(.opacity)
        }
    }

    @ViewBuilder
    private var editorTools: some View {
        if let gate = model.secretGate {
            SecretGateView(model: model, gate: gate)
        } else {
            tools
        }
    }

    private var tools: some View {
        VStack(alignment: .leading, spacing: DetailMetrics.stickRowGap) {
            if model.draft.needsAName {
                Text(
                    tr(
                        "A secret needs a name of its own — a name can be searched, so this one cannot be borrowed from what you typed.",
                        "密钥要自己起个名字——名字是可以被搜到的,所以不能从正文里借。"
                    )
                )
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink2)
            }
            HStack {
                StickVerb(title: tr("Cancel", "取消")) { model.cancelEditing() }
                Spacer(minLength: 0)
                StickVerb(title: saveVerb, isPrimary: true, action: onSave)
                    .disabled(!model.draft.canSave)
                    .opacity(model.draft.canSave ? 1 : DetailMetrics.disabledOpacity)
            }
            field(tr("Kind", "类型")) {
                // The marks stay the size the frames draw them; what a thumb
                // has to hit does not have to be the mark. Each takes an equal
                // share of the row and the platform's full minimum height, so
                // eight 22pt squares become eight targets a finger can land
                // on without aiming.
                HStack(spacing: 0) {
                    ForEach(kinds, id: \.self) { sort in
                        Button { model.draft.sort = sort } label: {
                            TypeSortMark(
                                sort,
                                size: .compact,
                                inverted: model.draft.sort == sort,
                                accessibilityLabel: sort.name(tr)
                            )
                            .frame(
                                maxWidth: .infinity,
                                minHeight: Tokens.Hit.minimum,
                                alignment: .center
                            )
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            field(tr("Trigger", "触发词")) {
                VStack(alignment: .leading, spacing: DetailMetrics.stickFieldGap) {
                    TextField("", text: $model.draft.trigger)
                        .typviaType(.mono)
                        .foregroundStyle(Room.home.accent)
                        .tint(Room.home.accent)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    refusalLine
                }
            }
            // Always offered, even with nothing to offer yet: a field that
            // only appears once a folder exists is a field nobody can ever
            // get their first folder from.
            field(tr("Folder", "文件夹")) { folderRow }
        }
    }

    private var kinds: [TypeSort] { Draft.offeredKinds(isNew: model.snippet == nil) }

    /// A secret is not saved, it is put away. The verb says which of the two
    /// is about to happen before it happens.
    private var saveVerb: String {
        model.snippet == nil && model.draft.effectiveSort == .secret
            ? tr("Put it away", "存进保险库")
            : tr("Save", "存下")
    }

    /// A refused save says so on the line under the field that caused it. No
    /// dialog, no red fill — the product's one modal is the vault's.
    @ViewBuilder
    private var refusalLine: some View {
        // While a folder is being named, a refusal is that folder's — it is
        // said under the field that caused it, not under this one.
        if let refusal = model.refusal, !isNamingFolder {
            Text(sentence(for: refusal))
                .typviaType(.caption)
                .foregroundStyle(Paper.attention)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func sentence(for refusal: Refusal) -> String {
        switch refusal {
        case .missing: tr("This snippet is no longer here.", "这枚片段已经不在了。")
        case .storage:
            tr("It could not be written just now. Nothing changed.", "刚才没写进去。什么都没有改动。")
        // On this screen a broken rule is nearly always the one trigger word
        // that is already somebody else's. The screen knows what the reader
        // was doing; the engine only knows a rule was broken.
        case .clash:
            tr(
                "That trigger word already belongs to another snippet.",
                "这个触发词已经是另一枚片段的了。"
            )
        default: refusal.sentence(tr)
        }
    }

    /// The folders this snippet could go in, and the way to make one that does
    /// not exist yet. Naming it is the whole of making it: the new folder is
    /// selected the moment it is named, because the reader was already filing
    /// something when they asked for it.
    private var folderRow: some View {
        VStack(alignment: .leading, spacing: DetailMetrics.stickFieldGap) {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: DetailMetrics.stickGap) {
                    ForEach(model.folders, id: \.id) { folder in
                        Button {
                            model.draft.folderId =
                                model.draft.folderId == folder.id ? nil : folder.id
                        } label: {
                            Text(folder.name)
                                .typviaType(.bodyS)
                                .foregroundStyle(
                                    model.draft.folderId == folder.id ? Paper.ink : Paper.ink3
                                )
                                .frame(minHeight: Tokens.Hit.minimum)
                        }
                        .buttonStyle(.plain)
                    }
                    if !isNamingFolder {
                        StickVerb(title: tr("New folder", "新建文件夹")) {
                            isNamingFolder = true
                        }
                    }
                }
            }
            if isNamingFolder {
                HStack(spacing: DetailMetrics.stickGap) {
                    TextField(
                        "",
                        text: $folderName,
                        prompt: Text(tr("Name it", "起个名字")).foregroundColor(Paper.ink3)
                    )
                    .typviaType(.bodyS)
                    .foregroundStyle(Paper.ink)
                    .tint(Room.home.accent)
                    .submitLabel(.done)
                    .onSubmit { makeFolder() }
                    StickVerb(title: tr("Make it", "建"), isPrimary: true, action: makeFolder)
                    StickVerb(title: tr("Cancel", "取消")) { stopNamingFolder() }
                }
                folderTroubleLine
            }
        }
    }

    @ViewBuilder
    private var folderTroubleLine: some View {
        if let refusal = model.refusal {
            Text(
                refusal == .invalid
                    ? tr("A folder needs a name.", "文件夹得有个名字。")
                    : refusal.sentence(tr)
            )
            .typviaType(.caption)
            .foregroundStyle(Paper.attention)
            .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func makeFolder() {
        let name = folderName
        Task {
            guard await model.makeFolder(named: name) else { return }
            stopNamingFolder()
        }
    }

    private func stopNamingFolder() {
        isNamingFolder = false
        folderName = ""
    }

    private func field(_ label: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: DetailMetrics.stickFieldGap) {
            Text(label)
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.ink3)
            content()
        }
    }
}

/// The AI strip: one line in the composing stick, in the clay room.
///
/// It never acts on its own. What comes back is a proposal the reader takes or
/// drops, and a snippet from the vault never reaches it — that refusal happens
/// behind the bridge, before a prompt exists.
struct AiStrip: View {
    @Environment(\.tr) private var tr
    @ObservedObject var model: DetailModel

    var body: some View {
        VStack(alignment: .leading, spacing: DetailMetrics.stickFieldGap) {
            switch model.ai {
            case .idle:
                idleRow
            case .working:
                HStack(spacing: DetailMetrics.stickRowGap) {
                    SweepLoader(accessibilityLabel: tr("Asking the model", "正在问模型"))
                    Text(tr("Asking the model", "正在问模型"))
                        .typviaType(.mono)
                        .foregroundStyle(Paper.ink3)
                }
            case let .suggested(suggestion):
                proposal(suggestion)
            case let .produced(text, maskedKinds):
                produced(text, maskedKinds)
            case let .refused(refusal):
                note(sentence(for: refusal))
            case .noEngine:
                note(
                    tr(
                        "No engine is set up — Settings 03 chooses one.",
                        "还没有选引擎——设置 03 里选一个。"
                    )
                )
            }
        }
        .room(.ai)
    }

    @ViewBuilder
    private var idleRow: some View {
        HStack(spacing: DetailMetrics.stickRowGap) {
            if model.mode != .reading {
                StickVerb(title: tr("Tidy this up", "帮我整理")) {
                    Task { await model.organize() }
                }
            }
            ForEach(model.actions) { action in
                StickVerb(title: action.name) { Task { await model.run(action) } }
            }
        }
    }

    /// What the model proposed, and the two words that decide its fate. It is
    /// applied only when the reader says so.
    private func proposal(_ suggestion: AiSuggestion) -> some View {
        VStack(alignment: .leading, spacing: DetailMetrics.stickFieldGap) {
            Text(describe(suggestion))
                .typviaType(.caption)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: DetailMetrics.stickGap) {
                StickVerb(title: tr("Take it", "采用"), isPrimary: true) {
                    model.accept(suggestion)
                }
                StickVerb(title: tr("Leave it", "不用")) { model.dismissAi() }
            }
        }
    }

    /// An action's output. Masked kinds are named: if something was hidden on
    /// the way out, the reader is told what kind of thing it was.
    private func produced(_ text: String, _ maskedKinds: [String]) -> some View {
        VStack(alignment: .leading, spacing: DetailMetrics.stickFieldGap) {
            Text(text)
                .typviaType(.mono)
                .foregroundStyle(Paper.ink)
                .lineLimit(DetailMetrics.outputLines)
                .fixedSize(horizontal: false, vertical: true)
            if !maskedKinds.isEmpty {
                Text(
                    tr(
                        "Masked before it left: \(maskedKinds.joined(separator: ", "))",
                        "出门前被遮掉的:\(maskedKinds.joined(separator: "、"))"
                    )
                )
                .typviaType(.mono)
                .foregroundStyle(Paper.attention)
                .fixedSize(horizontal: false, vertical: true)
            }
            HStack(spacing: DetailMetrics.stickGap) {
                StickVerb(title: tr("Use this", "用这段"), isPrimary: true) {
                    model.takeOutput(text)
                }
                StickVerb(title: tr("Leave it", "不用")) { model.dismissAi() }
            }
        }
    }

    private func note(_ sentence: String) -> some View {
        HStack(spacing: DetailMetrics.stickRowGap) {
            Text(sentence)
                .typviaType(.caption)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)
            StickVerb(title: tr("Close", "知道了")) { model.dismissAi() }
        }
    }

    private func describe(_ suggestion: AiSuggestion) -> String {
        var parts: [String] = []
        if let title = suggestion.title { parts.append(tr("title \(title)", "标题「\(title)」")) }
        if let type = suggestion.snippetType, let sort = TypeSort(coreType: type) {
            parts.append(tr("kind \(sort.name(tr))", "类型\(sort.name(tr))"))
        }
        if let trigger = suggestion.trigger { parts.append(tr("trigger \(trigger)", "触发词 \(trigger)")) }
        if !suggestion.tags.isEmpty {
            parts.append(tr("tags \(suggestion.tags.joined(separator: ", "))", "标签 \(suggestion.tags.joined(separator: "、"))"))
        }
        return tr("Proposed: ", "建议:") + parts.joined(separator: tr(" · ", " · "))
    }

    private func sentence(for refusal: Refusal) -> String {
        AiStripCopy.refusal(refusal, tr)
    }
}

/// One word in the composing stick. Words, not buttons with borders.
struct StickVerb: View {
    @Environment(\.room) private var room
    @Environment(\.isEnabled) private var isEnabled

    let title: String
    var isPrimary = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .typviaType(isPrimary ? .sectionTitle : .bodyS)
                .foregroundStyle(ink)
                .padding(.horizontal, isPrimary ? VerbMetrics.padX : 0)
                .frame(minHeight: Tokens.Hit.minimum)
                .background {
                    // A key action is a shape, not a word among words. The
                    // delivery draws verbs as plain text and this product did
                    // exactly that — and the reader could not tell, on any
                    // screen, what was a control and what was a sentence. So
                    // the one act each page exists for gets a fill; everything
                    // else stays a word, because a page where everything is a
                    // button is the same problem the other way round.
                    if isPrimary {
                        RoundedRectangle(
                            cornerRadius: Tokens.Radius.control,
                            style: .continuous
                        )
                        .fill(isEnabled ? room.accent : Paper.carrier)
                    }
                }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    /// On a fill, the type is the paper it sits on rather than a lighter ink —
    /// the accents are dark enough that ink on ink would not read.
    private var ink: Color {
        guard isPrimary else { return Paper.ink2 }
        return isEnabled ? Paper.base : Paper.ink3
    }
}

enum VerbMetrics {
    static let padX: CGFloat = 18
}

/// The explanation a page owes but should not open with.
///
/// The chapters were written as essays: every fact stated, every reason given,
/// all of it at once. That is right for the reader who wants it and wrong for
/// the one who came to do something — and it is what made the pages read as
/// walls of type with no way to tell a verb from a sentence.
///
/// So the reasons move behind one word. Nothing is deleted: what a page knows
/// it still says, and says in full, one tap away.
struct Aside<Content: View>: View {
    @State private var isOpen = false

    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            MoreWord(isOpen: $isOpen)
            if isOpen {
                content
            }
        }
    }
}

/// The word that opens an aside.
///
/// Its own view because some pages interleave their reasons with their facts
/// rather than stacking them at the end — those hold the state themselves and
/// still print the same word in the same place.
struct MoreWord: View {
    @Environment(\.tr) private var tr
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @Binding var isOpen: Bool

    var body: some View {
        Button {
            // The delivery's own beat for a disclosure — a state changing in
            // place, not a page arriving.
            withAnimation(Beat.state.enter(reduceMotion: reduceMotion)) { isOpen.toggle() }
        } label: {
            Text(isOpen ? tr("Less", "收起") : tr("More", "更多"))
                .typviaType(.mono)
                .foregroundStyle(Paper.ink3)
                .frame(minHeight: Tokens.Hit.minimum)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(isOpen ? [.isSelected] : [])
    }
}

/// The editor's body field, on whichever material the chosen kind wears.
struct ComposerBody: View {
    @Environment(\.tr) private var tr
    @ObservedObject var model: DetailModel

    let material: AnyView?
    /// Owned a level up, because the screen has to know when this is being
    /// typed into: the keyboard covers the bottom half and something has to
    /// bring the caret back into view.
    var isFocused: FocusState<Bool>.Binding?

    init(model: DetailModel, material: some View, isFocused: FocusState<Bool>.Binding? = nil) {
        self.model = model
        self.material = AnyView(material)
        self.isFocused = isFocused
    }

    var body: some View {
        ZStack(alignment: .topLeading) {
            material
            // Gone the moment there is a caret. It used to look only at
            // whether the body was empty, so the hint and the caret were drawn
            // on top of each other — and it sat at zero while the editor's own
            // text starts inset, so the two were not even aligned.
            if model.draft.body.isEmpty, isFocused?.wrappedValue != true {
                Text(tr("Paste what you want to keep.", "把要存的文字贴进来。"))
                    .typviaType(.mono)
                    .foregroundStyle(Paper.ink3)
                    .padding(.leading, DetailMetrics.editorInsetX)
                    .padding(.top, DetailMetrics.editorInsetY)
                    .allowsHitTesting(false)
            }
            editor
        }
    }

    @ViewBuilder
    private var editor: some View {
        let field = TextEditor(text: $model.draft.body)
            .typviaType(.mono)
            .foregroundStyle(Paper.ink)
            .tint(Room.home.accent)
            .scrollContentBackground(.hidden)
            .frame(minHeight: DetailMetrics.composerMinHeight)
        if let isFocused {
            field.focused(isFocused)
        } else {
            field
        }
    }
}

/// A template, read: the words with the blanks standing in them as pale slips
/// of paper rather than as bracket syntax nobody should have to decode.
///
/// With fields to fill, the blanks become a short list under the sentence and
/// the sentence updates as they are typed — the preview is the shared engine's
/// answer, re-asked on every keystroke, not a local substitution.
struct TemplateBodyView: View {
    @Environment(\.tr) private var tr
    @ObservedObject var model: DetailModel

    let template: TemplateBody

    var body: some View {
        VStack(alignment: .leading, spacing: DetailMetrics.stickRowGap) {
            WrappingPieces(pieces: template.pieces)
            if !model.fields.isEmpty {
                VStack(alignment: .leading, spacing: DetailMetrics.stickFieldGap) {
                    ForEach(model.fields, id: \.id) { field in
                        blank(field)
                    }
                }
            }
        }
    }

    private func blank(_ field: TemplateField) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: DetailMetrics.stickRowGap) {
            Text(field.label.isEmpty ? field.name : field.label)
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.ink3)
                .frame(width: DetailMetrics.blankLabelWidth, alignment: .leading)
            TextField(
                "",
                text: Binding(
                    get: { model.values[field.name] ?? "" },
                    set: { model.values[field.name] = $0 }
                ),
                prompt: Text(field.isRequired ? tr("required", "必填") : tr("optional", "选填"))
                    .foregroundColor(Paper.ink3)
            )
            .typviaType(.mono)
            .foregroundStyle(Room.library.accent)
            .tint(Room.library.accent)
        }
        .padding(.vertical, DetailMetrics.blankPaddingY)
        .padding(.horizontal, DetailMetrics.blankPaddingX)
        // A pale paper block, as the delivery draws it — not an underlined
        // field. The two numbers for it were in the metrics from the start and
        // never used: the blank had been built as a rule under a line, which
        // reads as a form rather than as a gap in a sentence.
        .background(
            RoundedRectangle(cornerRadius: DetailMetrics.blankRadius)
                .fill(Paper.carrier)
        )
    }
}

private struct WrappingPieces: View {
    let pieces: [TemplateBody.Piece]

    var body: some View {
        // Text concatenation keeps the sentence a sentence: the blanks flow
        // with the words and wrap with them, rather than becoming a row of
        // chips beside the prose.
        pieces.reduce(Text("")) { running, piece in
            switch piece {
            case let .text(text):
                return running + Text(text)
            case let .blank(name):
                return running + Text(" \(name) ")
                    .foregroundColor(Room.library.accent)
            }
        }
        .typviaType(.mono)
        .foregroundStyle(Paper.ink)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// A secret's body: frosted, dotted, and only real for as long as it is out.
struct SecretBody: View {
    @Environment(\.tr) private var tr
    @ObservedObject var model: DetailModel

    /// Fixed lengths. Dotted lines that followed the real body would describe
    /// the thing they are covering.
    private static let ruleWidths: [CGFloat] = [0.92, 0.68, 0.84, 0.44]

    var body: some View {
        VStack(alignment: .leading, spacing: DetailMetrics.secretLineGap) {
            if let plaintext = model.revealed {
                Text(plaintext)
                    .typviaType(.mono)
                    .foregroundStyle(VaultRoom.ink)
                    .textSelection(.enabled)
                countdown
            } else {
                ForEach(Array(SecretBody.ruleWidths.enumerated()), id: \.offset) { _, width in
                    GeometryReader { proxy in
                        Text(BodyPreview.mask)
                            .typviaType(.mono)
                            .foregroundStyle(VaultRoom.ink3)
                            .lineLimit(1)
                            .fixedSize(horizontal: true, vertical: false)
                            .frame(width: proxy.size.width * width, alignment: .leading)
                            .clipped()
                    }
                    .frame(height: DetailMetrics.secretLineGap)
                    .accessibilityHidden(true)
                }
                Text(
                    tr(
                        "The body appears only while it is out, and covers itself again after.",
                        "正文只在取出的那一刻出现,取完即遮回。"
                    )
                )
                .typviaType(.bodyS)
                .foregroundStyle(VaultRoom.ink2)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, DetailMetrics.secretLineGap)
                Button { Task { await model.revealSecret() } } label: {
                    Text(tr("Take it out with Face ID", "用面容 ID 取出"))
                        .typviaType(.sectionTitle)
                        .foregroundStyle(VaultRoom.accent)
                        .frame(minHeight: Tokens.Hit.minimum)
                }
                .buttonStyle(.plain)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// The window running out, as a rule that shrinks. No digits: a number
    /// counting down on a secret reads as a threat.
    @ViewBuilder
    private var countdown: some View {
        if case let .shown(secondsLeft) = model.reveal {
            Rectangle()
                .fill(VaultRoom.accent)
                .frame(height: DetailMetrics.countdownHeight)
                .scaleEffect(x: Reveal.remaining(secondsLeft: secondsLeft), anchor: .leading)
                .accessibilityLabel(tr("Hides itself shortly", "稍后自动遮回"))
        }
    }
}

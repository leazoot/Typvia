// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// One snippet's screen: reading it, editing it, or writing a new one.
///
/// Validity is not decided here. A trigger that collides, a body that is too
/// long, a kind that does not exist — the core answers all of those, and what
/// this does with the answer is put it on the line under the field rather than
/// in a dialog.
@MainActor
public final class DetailModel: ObservableObject {
    @Published public private(set) var snippet: Snippet?
    @Published public private(set) var mode: DetailMode
    @Published public private(set) var folders: [FolderRef] = []
    @Published public private(set) var templateBody: TemplateBody?
    /// A template's blanks, and what has been typed into them. The values live
    /// here only while the screen does — a filled template is not saved, it is
    /// inserted.
    @Published public private(set) var fields: [TemplateField] = []
    @Published public var values: [String: String] = [:] {
        didSet {
            guard values != oldValue else { return }
            Task { await refreshPreview() }
        }
    }
    /// A refusal, shown under the field it belongs to. Never a dialog: the
    /// whole product has one modal and this is not it.
    @Published public private(set) var refusal: Refusal?
    /// Set the moment a save lands, so the screen can press the words into the
    /// paper and say so.
    @Published public private(set) var savedAt: Date?
    @Published public var draft: Draft

    /// The AI strip: what has been asked, and what came back.
    @Published public private(set) var ai: AiStripPhase = .idle
    /// The actions whose contract says they may run on a snippet's body.
    @Published public private(set) var actions: [AiActionRow] = []

    /// The decrypted body of a secret, for as long as it is out. Nothing else
    /// holds it: not the draft, not the snippet, not a cache.
    @Published public private(set) var revealed: String?
    @Published public private(set) var reveal: Reveal = .hidden

    /// What the vault needs before this draft can become a secret, if it is
    /// one. Nil at every other moment, including while a secret is being typed.
    @Published public private(set) var secretGate: SecretGate?
    /// Set when the two typed master passwords differ. It is not a refusal
    /// from anywhere: nothing was sent, and the vault does not know a second
    /// field exists.
    @Published public private(set) var mismatch = false

    private let store: TypviaStore
    private let snippetId: String?
    private var hideTask: Task<Void, Never>?
    /// Set while a saved snippet is waiting on the vault to become a secret.
    /// It is what tells the gate which act it is standing in front of.
    private var pendingPromotion: String?

    /// Opens an existing snippet for reading.
    public init(store: TypviaStore, snippetId: String) {
        self.store = store
        self.snippetId = snippetId
        mode = .reading
        draft = Draft()
    }

    /// Opens the editor on a snippet that does not exist yet.
    ///
    /// - Parameter sort: the kind already chosen, for the rooms that lead here
    ///   knowing what the reader came to write. The vault room needs it: on
    ///   this platform a vault is made by putting a secret away, so "make a
    ///   vault" and "write a secret" are the same act, and arriving at a blank
    ///   editor with eight kinds to pick from hides that.
    public init(store: TypviaStore, clipboard: String? = nil, sort: TypeSort? = nil) {
        self.store = store
        snippetId = nil
        mode = .creating
        draft = Draft(body: clipboard ?? "", sort: sort)
    }

    public var isSecret: Bool {
        snippet.map { $0.securityLevel != SecurityLevelCode.normal } ?? false
    }

    public func load() async {
        guard let snippetId else {
            folders = await readFolders()
            return
        }
        let loaded = try? await store.perform { try $0.snippetGet(id: snippetId) }
        guard let loaded else {
            refusal = .missing
            return
        }
        snippet = loaded
        draft = Draft(snippet: loaded)
        folders = await readFolders()
        await readTemplate(loaded)
    }

    /// Reads the saved actions. Only the ones whose contract accepts a
    /// snippet's body are offered — an action is a promise about where its
    /// text comes from, and offering one it cannot keep is worse than not
    /// offering it.
    public func loadActions() async {
        let rows = try? await store.perform { core in
            try core.aiActionList().map(AiActionRow.init(action:))
        }
        actions = (rows ?? []).filter(\.canRunHere)
    }

    /// Asks the model to propose a title, kind and trigger for what is being
    /// written. Nothing is applied: the strip proposes, the reader disposes.
    public func organize() async {
        guard !isSecret else { return }
        guard let provider = await firstProvider() else {
            ai = .noEngine
            return
        }
        ai = .working
        do {
            let draft = draft
            let suggestion = try await store.perform { core in
                try core.aiOrganize(
                    providerId: provider,
                    title: draft.title,
                    body: draft.body,
                    description: nil,
                    // Passed through, never decided here. The gate that
                    // refuses a sensitive draft runs before a prompt exists.
                    isSensitive: false
                )
            }
            let proposal = AiSuggestion(suggestion)
            ai = proposal.isEmpty ? .idle : .suggested(proposal)
        } catch let error as CoreError {
            ai = .refused(Refusal(error))
        } catch {
            ai = .refused(.storage)
        }
    }

    /// Runs one action on this snippet's body.
    public func run(_ action: AiActionRow) async {
        guard let snippet, let body = snippet.body, !isSecret else { return }
        ai = .working
        do {
            let result = try await store.perform { core in
                try core.aiActionRun(
                    actionId: action.id,
                    text: body,
                    source: AiActionRow.source,
                    isSensitive: false
                )
            }
            ai = .produced(text: result.output, maskedKinds: result.maskedKinds)
        } catch let error as CoreError {
            ai = .refused(Refusal(error))
        } catch {
            ai = .refused(.storage)
        }
    }

    /// Takes a suggestion into the draft. Only the parts it actually proposed.
    public func accept(_ suggestion: AiSuggestion) {
        if let title = suggestion.title, !title.isEmpty { draft.title = title }
        if let type = suggestion.snippetType, let sort = TypeSort(coreType: type) {
            draft.sort = sort
        }
        if let trigger = suggestion.trigger, !trigger.isEmpty { draft.trigger = trigger }
        ai = .idle
    }

    /// Replaces the body with what an action produced.
    public func takeOutput(_ text: String) {
        draft.body = text
        ai = .idle
    }

    public func dismissAi() {
        ai = .idle
    }

    private func firstProvider() async -> String? {
        let providers = try? await store.perform { try $0.aiProviderList() }
        return providers?.first?.id
    }

    public func edit() {
        guard !isSecret else { return }
        refusal = nil
        mode = .editing
    }

    public func cancelEditing() {
        refusal = nil
        if let snippet {
            draft = Draft(snippet: snippet)
            mode = .reading
        }
    }

    /// Saves, and reports a refusal where the reader can act on it.
    ///
    /// - Returns: whether something was written. A draft that opened the vault
    ///   gate has not been written yet, and the screen must not celebrate it.
    @discardableResult
    public func save() async -> Bool {
        guard draft.canSave else { return false }
        refusal = nil
        // A secret goes through the vault's own door. The ordinary one refuses
        // the kind outright — it would store the body in the clear under a
        // name that says otherwise — so this is the only way to write one.
        if snippet == nil, draft.effectiveSort == .secret {
            return await saveSecret()
        }
        let draft = draft
        let existing = snippet
        do {
            let saved = try await store.perform { core -> Snippet in
                if let existing {
                    return try core.snippetUpdate(
                        edit: SnippetEdit(
                            id: existing.id,
                            title: DetailModel.name(for: draft),
                            body: draft.body,
                            snippetType: draft.effectiveSort.coreType,
                            description: existing.description,
                            folderId: draft.folderId,
                            trigger: DetailModel.trigger(for: draft),
                            // What a trigger means when nobody says is the
                            // shared layer's answer now, not this screen's.
                            triggerMode: nil,
                            language: existing.language,
                            isFavorite: existing.isFavorite,
                            isPinned: existing.isPinned,
                            isEnabled: existing.isEnabled
                        )
                    )
                }
                return try core.snippetCreate(
                    draft: SnippetDraft(
                        title: DetailModel.name(for: draft),
                        body: draft.body,
                        snippetType: draft.effectiveSort.coreType,
                        description: nil,
                        folderId: draft.folderId,
                        trigger: DetailModel.trigger(for: draft),
                        triggerMode: nil,
                        language: nil
                    )
                )
            }
            adopt(saved)
            await readTemplate(saved)
            return true
        } catch let error as CoreError {
            refusal = Refusal(error)
        } catch {
            refusal = .storage
        }
        return false
    }

    /// Turns a snippet that is already saved into a secret.
    ///
    /// A different act from choosing a kind, and a heavier one: the body is
    /// re-encrypted, the index is rebuilt so the former plaintext leaves index
    /// storage, and the plaintext version history is dropped. A secret must not
    /// leave its own cleartext past behind — which is why this goes through the
    /// core's own promotion rather than through an edit.
    @discardableResult
    public func promoteToSecret() async -> Bool {
        guard let snippet, !isSecret else { return false }
        refusal = nil
        pendingPromotion = snippet.id
        guard let status = try? await store.perform({ try $0.vaultStatus() }) else {
            refusal = .storage
            return false
        }
        guard status.initialized else {
            secretGate = .setup
            return false
        }
        guard status.unlocked else {
            secretGate = .shut
            return false
        }
        return await promote()
    }

    private func promote() async -> Bool {
        guard let id = pendingPromotion else { return false }
        do {
            let promoted = try await store.perform { try $0.snippetConvertToSensitive(id: id) }
            pendingPromotion = nil
            secretGate = nil
            adopt(promoted)
            return true
        } catch let error as CoreError {
            refusal = Refusal(error)
        } catch {
            refusal = .storage
        }
        return false
    }

    /// Writes a secret, or says what the vault needs first.
    private func saveSecret() async -> Bool {
        guard let status = try? await store.perform({ try $0.vaultStatus() }) else {
            refusal = .storage
            return false
        }
        guard status.initialized else {
            secretGate = .setup
            return false
        }
        guard status.unlocked else {
            secretGate = .shut
            return false
        }
        return await writeSecret()
    }

    /// Makes the vault this device does not have, then writes the secret that
    /// asked for it.
    ///
    /// The confirmation is checked here and nowhere else: the vault is handed
    /// one password, because a second one is a question about typing rather
    /// than about the key.
    @discardableResult
    public func makeVault(password: String, confirmation: String) async -> Bool {
        refusal = nil
        mismatch = false
        guard password == confirmation else {
            mismatch = true
            return false
        }
        do {
            _ = try await store.perform { try $0.vaultInitialize(password: password) }
        } catch let error as CoreError {
            refusal = Refusal(error)
            return false
        } catch {
            refusal = .storage
            return false
        }
        return await finishWhateverAskedForTheVault()
    }

    /// Whatever opened the gate is what happens once it is through: a draft
    /// waiting to be written, or a snippet waiting to be promoted.
    private func finishWhateverAskedForTheVault() async -> Bool {
        pendingPromotion == nil ? await writeSecret() : await promote()
    }

    /// Opens the shut vault with the master password, then writes.
    @discardableResult
    public func openVault(password: String) async -> Bool {
        await open { try $0.vaultUnlockPassword(password: password) }
    }

    /// Says that the words were actually handed over.
    ///
    /// The host does the handing — a clipboard, a text field — and this says
    /// so afterwards, never before: the recall shelf is built from these, and
    /// counting a delivery that did not happen is a shelf recommending things
    /// nobody used. A failure here is not worth telling the reader about; the
    /// words are already where they wanted them.
    public func recordUse() async {
        guard let id = snippetId else { return }
        _ = try? await store.perform { try $0.snippetRecordUse(id: id) }
    }

    /// Opens it with the platform's own gate. Reading the master-key copy is
    /// itself the Face ID prompt; there is no separate check to pass here.
    @discardableResult
    public func openVaultWithBiometrics() async -> Bool {
        await open { try $0.vaultUnlockBiometric() }
    }

    private func open(_ unlock: @escaping @Sendable (TypviaCore) throws -> VaultStatus) async -> Bool {
        refusal = nil
        do {
            _ = try await store.perform(unlock)
        } catch let error as CoreError {
            refusal = Refusal(error)
            return false
        } catch {
            refusal = .storage
            return false
        }
        return await writeSecret()
    }

    /// Leaves the gate without writing. The draft is untouched — a reader who
    /// changes their mind about the vault has not changed their mind about
    /// what they wrote.
    public func dismissGate() {
        secretGate = nil
        mismatch = false
        refusal = nil
        pendingPromotion = nil
    }

    private func writeSecret() async -> Bool {
        let draft = draft
        do {
            let saved = try await store.perform { core in
                try core.vaultCreateSecret(
                    draft: SnippetDraft(
                        title: DetailModel.name(for: draft),
                        body: draft.body,
                        snippetType: draft.effectiveSort.coreType,
                        description: nil,
                        folderId: draft.folderId,
                        trigger: DetailModel.trigger(for: draft),
                        triggerMode: nil,
                        language: nil
                    )
                )
            }
            secretGate = nil
            mismatch = false
            adopt(saved)
            return true
        } catch let error as CoreError {
            refusal = Refusal(error)
        } catch {
            refusal = .storage
        }
        return false
    }

    /// Takes what was written as the screen's new subject.
    ///
    /// The draft is rebuilt from the saved row rather than kept, which for a
    /// secret is the moment the typed plaintext leaves this object: the row
    /// that comes back carries no body, so neither does the draft.
    private func adopt(_ saved: Snippet) {
        snippet = saved
        draft = Draft(snippet: saved)
        mode = .reading
        savedAt = Date()
    }

    /// Takes a secret out for the length of the window, then puts it back.
    ///
    /// The plaintext crosses the bridge once, is held only while it is on
    /// screen, and is dropped by the same code path whether the window ran out
    /// or the reader left.
    public func revealSecret() async {
        guard let snippetId, isSecret, revealed == nil else { return }
        do {
            let body = try await store.perform { try $0.vaultReveal(id: snippetId) }
            revealed = body
            reveal = .shown(secondsLeft: Reveal.window)
            Haptic.success.play()
            startHideCountdown()
        } catch let error as CoreError {
            refusal = Refusal(error)
        } catch {
            refusal = .storage
        }
    }

    /// Puts a revealed secret back under. Called by the countdown, by leaving
    /// the screen, and by the reader.
    public func hideSecret() {
        hideTask?.cancel()
        hideTask = nil
        revealed = nil
        reveal = .hidden
    }

    private func startHideCountdown() {
        hideTask?.cancel()
        hideTask = Task { [weak self] in
            let step: TimeInterval = 0.1
            var left = Reveal.window
            while left > 0 {
                try? await Task.sleep(nanoseconds: UInt64(step * 1_000_000_000))
                guard !Task.isCancelled else { return }
                left -= step
                self?.reveal = .shown(secondsLeft: max(left, 0))
            }
            self?.hideSecret()
        }
    }

    /// Makes a folder and files this draft in it.
    ///
    /// The name is handed over as typed. Whether it is a name at all — after
    /// trimming, whether anything is left — is the core's answer, the same one
    /// the desktop gets, and it lands on the line under the field like every
    /// other refusal on this screen.
    @discardableResult
    public func makeFolder(named name: String) async -> Bool {
        refusal = nil
        do {
            let folder = try await store.perform { core in
                try core.folderCreate(name: name, parentId: nil, sortOrder: 0)
            }
            folders = await readFolders()
            // Selected on the way out: asking for a folder while filing
            // something is asking for that thing to go in it.
            draft.folderId = folder.id
            return true
        } catch let error as CoreError {
            refusal = Refusal(error)
        } catch {
            refusal = .storage
        }
        return false
    }

    private func readFolders() async -> [FolderRef] {
        let rows = try? await store.perform { core in
            try core.folderListChildren(parentId: nil)
                .map { FolderRef(id: $0.id, name: $0.name) }
        }
        return rows ?? []
    }

    /// Asks the shared engine what this template looks like with nothing filled
    /// in. The blanks come back marked; splitting them out is presentation.
    private func readTemplate(_ snippet: Snippet) async {
        guard TypeSort(coreType: snippet.snippetType) == .template, let body = snippet.body else {
            templateBody = nil
            return
        }
        let read = try? await store.perform { core -> (String, [TemplateField]) in
            var fields = try core.templateFields(snippetId: snippet.id)
            if fields.isEmpty {
                // A template that has never been opened has no field records
                // yet. The names come from the shared engine — Swift does not
                // know what a blank looks like — and are written down once so
                // that rendering, which needs them, has them.
                let names = try core.templateVariables(body: body)
                if !names.isEmpty {
                    fields = try core.templateSaveFields(
                        snippetId: snippet.id,
                        fields: names.enumerated().map { index, name in
                            TemplateField(
                                id: UUID().uuidString,
                                name: name,
                                label: name,
                                // The engine's vocabulary, not a guess: an
                                // unknown type is a validation refusal.
                                fieldType: "single_line_text",
                                defaultValue: nil,
                                options: [],
                                validation: nil,
                                isRequired: true,
                                sortOrder: Int32(index),
                                platformOverrides: nil
                            )
                        }
                    )
                }
            }
            return (try core.templatePreview(body: body, fields: fields, values: [:]), fields)
        }
        guard let read else {
            templateBody = nil
            fields = []
            return
        }
        templateBody = TemplateBody(preview: read.0)
        fields = read.1
    }

    /// Re-asks the engine what the template looks like with what has been
    /// typed so far. The preview is the engine's, not a local substitution:
    /// there is one set of template rules and it is not in Swift.
    private func refreshPreview() async {
        guard let snippet, let body = snippet.body, !fields.isEmpty else { return }
        let fields = fields
        let values = values
        let preview = try? await store.perform { core in
            try core.templatePreview(body: body, fields: fields, values: values)
        }
        templateBody = preview.map(TemplateBody.init(preview:))
    }

    /// The filled text, ready to go into something. A missing required field
    /// is the engine's refusal, not a local check — and it lands on the line
    /// under the fields like every other refusal on this screen.
    public func filled() async -> String? {
        guard let snippet else { return nil }
        let values = values
        do {
            return try await store.perform { try $0.templateRender(id: snippet.id, values: values) }
        } catch let error as CoreError {
            refusal = Refusal(error)
            return nil
        } catch {
            refusal = .storage
            return nil
        }
    }

    /// The title as typed. Naming an untitled snippet after its first line is
    /// the shared layer's job now — it was here, and the other platform was
    /// about to hold a second copy of it.
    nonisolated static func name(for draft: Draft) -> String {
        draft.title.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    nonisolated static func trigger(for draft: Draft) -> String? {
        let trimmed = draft.trigger.trimmingCharacters(in: .whitespaces)
        return trimmed.isEmpty ? nil : trimmed
    }

}


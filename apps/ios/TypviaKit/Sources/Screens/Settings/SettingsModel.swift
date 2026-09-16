// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// The settings room's state: what each chapter currently reports.
///
/// Every value here is read from the core or from the sync host. Nothing on
/// this screen keeps its own copy of a setting — a settings screen that
/// remembers what it thinks the truth is eventually disagrees with it.
@MainActor
public final class SettingsModel: ObservableObject {
    @Published public private(set) var open: SettingsChapter?
    @Published public private(set) var devices: [DeviceRow] = []
    @Published public private(set) var engine: AiEngine = .off
    @Published public private(set) var stats = DataStats(total: 0, sorts: TypeSort.allCases.count)
    @Published public private(set) var trouble: SyncTrouble?
    @Published public private(set) var isRetrying = false
    @Published public private(set) var vault = VaultFacts(isMade: false, idleTimeoutMs: 0)
    /// What the vault chapter's last act did, and why it did not.
    @Published public private(set) var vaultSaid: String?
    @Published public private(set) var vaultRefusal: Refusal?
    @Published public private(set) var lastSyncAt: Int64?
    @Published public private(set) var trash: [TrashRow] = []
    /// The engine's refusal, shown under the field it belongs to.
    @Published public private(set) var refusal: Refusal?
    /// What the data chapter's last act did, and why it did not.
    @Published public private(set) var dataSaid: String?
    @Published public private(set) var dataRefusal: DataRefusal?

    private let store: TypviaStore

    public init(store: TypviaStore) {
        self.store = store
    }

    public func load() async {
        let snapshot = try? await store.perform { core -> Snapshot in
            let counts = try core.libraryCounts()
            let vault = try core.vaultStatus()
            let sync = try? core.syncStatus()
            let devices = (try? core.syncDevices()) ?? []
            let providers = (try? core.aiProviderList()) ?? []
            return Snapshot(
                total: counts.total,
                vault: VaultFacts(isMade: vault.initialized, idleTimeoutMs: vault.idleTimeoutMs),
                enabled: sync?.enabled ?? false,
                queued: sync.map { $0.enabled ? $0.pendingBacklog : 0 } ?? 0,
                lastSyncAt: sync?.lastSyncAt,
                devices: devices.map {
                    DeviceRow(
                        device: $0,
                        pending: sync.map { $0.enabled ? $0.pendingBacklog : 0 } ?? 0
                    )
                },
                engine: AiEngine.inEffect(providers: providers)
            )
        }
        guard let snapshot else { return }
        stats = DataStats(total: snapshot.total, sorts: TypeSort.allCases.count)
        vault = snapshot.vault
        lastSyncAt = snapshot.lastSyncAt
        engine = snapshot.engine
        devices = snapshot.devices
        trouble = snapshot.queued > 0
            ? SyncTrouble(
                queued: snapshot.queued, lastSyncAt: snapshot.lastSyncAt, total: snapshot.total
            )
            : nil
    }

    /// Reads the recycle bin. Only the data chapter asks, and it asks when it
    /// opens — a count that is stale by a screen is a count that misleads.
    public func loadTrash() async {
        let rows = try? await store.perform { core in
            try core.trashList(limit: 100, offset: 0).map(TrashRow.init(snippet:))
        }
        trash = rows ?? []
    }

    public func putBack(_ id: String) async {
        try? await store.perform { try $0.snippetRestore(id: id) }
        await loadTrash()
        await load()
    }

    /// Permanent, and the only irreversible thing this room does. The
    /// confirmation is the view's; what happens here is what the word says.
    public func deleteForever(_ id: String) async {
        try? await store.perform { try $0.snippetDeleteForever(id: id) }
        await loadTrash()
        await load()
    }

    // MARK: - The library's way out and back in
    //
    // What a format is, what may be imported, what a backup holds and what
    // restoring one requires are the core's answers. These carry the question
    // across and keep what came back — including the refusals, which this page
    // must state rather than swallow: it is the one screen where the reader's
    // whole library is on the table.

    /// Seals the whole library and hands back the file's text.
    ///
    /// - Returns: nil when it was refused; the refusal is published.
    /// - Note: the passphrase crosses once, derives the key behind the bridge
    ///   and is dropped. Nothing here keeps a copy of it.
    public func exportEverything(passphrase: String) async -> String? {
        dataRefusal = nil
        dataSaid = nil
        do {
            return try await store.perform { try $0.backupExport(passphrase: passphrase) }
        } catch let error as CoreError {
            dataRefusal = DataRefusal(error)
        } catch {
            dataRefusal = .storage
        }
        return nil
    }

    /// Says what the export became, once the file has actually been written.
    /// Said after the fact rather than before: a page that congratulates
    /// itself before the file exists is a page that lies when the write fails.
    public func exportLanded(_ tr: Translator) {
        dataSaid = tr(
            "Everything is in that file, sealed with the passphrase you typed. Nothing but you and that passphrase opens it.",
            "所有东西都在那个文件里,用你刚输入的口令封着。除了你和那句口令,没有别的能打开它。"
        )
    }

    public func refuse(_ refusal: DataRefusal) {
        dataSaid = nil
        dataRefusal = refusal
    }

    public func clearDataReport() {
        dataSaid = nil
        dataRefusal = nil
    }

    public func bringIn(format: String, text: String, _ tr: Translator) async {
        dataRefusal = nil
        do {
            let report = try await store.perform {
                try $0.snippetsImport(format: format, text: text)
            }
            dataSaid = DataChapterCopy.imported(report, tr)
            await load()
        } catch let error as CoreError {
            dataRefusal = DataRefusal(error)
        } catch {
            dataRefusal = .storage
        }
    }

    public func putItAllBack(passphrase: String, text: String, _ tr: Translator) async {
        dataRefusal = nil
        do {
            let report = try await store.perform {
                try $0.backupRestore(passphrase: passphrase, text: text)
            }
            dataSaid = DataChapterCopy.restored(report, tr)
            await load()
        } catch let error as CoreError {
            dataRefusal = DataRefusal(error)
        } catch {
            dataRefusal = .storage
        }
    }

    // MARK: - How the vault opens
    //
    // Keeping a Face ID copy of the master key needs the key in hand, and the
    // vault is always shut in this room: leaving the vault locks it. So the
    // master password is asked for here, handed straight to the bridge, and
    // the vault is shut again in the same breath — a settings page must not
    // leave an open vault behind it.
    //
    // Neither act reports a state afterwards, because none can be read: the
    // secure store answers reads, not questions about what it holds, and a
    // read of the gated entry *is* the Face ID sheet. What these say is what
    // just happened.

    /// Keeps a copy of the master key that Face ID opens.
    public func useFaceId(password: String, _ tr: Translator) async {
        vaultSaid = nil
        vaultRefusal = nil
        do {
            _ = try await store.perform { try $0.vaultUnlockPassword(password: password) }
        } catch let error as CoreError {
            vaultRefusal = Refusal(error)
            return
        } catch {
            vaultRefusal = .storage
            return
        }
        do {
            try await store.perform { try $0.vaultEnableBiometric() }
            vaultSaid = tr(
                "Face ID opens the vault now. The master password still does too.",
                "现在面容 ID 能打开保险库了。主密码照样也能。"
            )
        } catch let error as CoreError {
            vaultRefusal = Refusal(error)
        } catch {
            vaultRefusal = .storage
        }
        // Shut whatever the enrolment did or did not manage. This runs even
        // where the enrolment failed: the vault was opened to try it.
        _ = try? await store.perform { try $0.vaultLock() }
        await load()
    }

    /// Takes that copy away. It needs no password: removing a key copy is not
    /// a way into anything, and the master password is untouched either way.
    public func stopUsingFaceId(_ tr: Translator) async {
        vaultSaid = nil
        vaultRefusal = nil
        do {
            try await store.perform { try $0.vaultDisableBiometric() }
            vaultSaid = tr(
                "That copy is gone. The master password opens the vault.",
                "那份副本没了。保险库用主密码打开。"
            )
        } catch let error as CoreError {
            vaultRefusal = Refusal(error)
        } catch {
            vaultRefusal = .storage
        }
    }

    public func clearVaultReport() {
        vaultSaid = nil
        vaultRefusal = nil
    }

    public func openChapter(_ chapter: SettingsChapter) {
        open = chapter
    }

    public func closeChapter() {
        open = nil
    }

    /// Puts the chosen engine into effect.
    ///
    /// The choice is not a preference this screen keeps — it is the set of
    /// providers, which is what everything else reads. Choosing "off" removes
    /// them; choosing a machine writes one. The API key never passes through
    /// here: it goes straight to the secure store behind the bridge.
    public func choose(
        _ engine: AiEngine,
        model: String = "",
        baseUrl: String = "",
        apiKey: String? = nil
    ) async {
        refusal = nil
        let existing = (try? await store.perform { try $0.aiProviderList() }) ?? []
        switch engine {
        case .off:
            // Removing a provider takes its key out of the secure store
            // first. Where that store cannot be reached the removal fails —
            // and the screen must not then show "off" over an engine that is
            // still configured. It says what happened and leaves the state
            // as it truly is.
            for provider in existing {
                do {
                    try await store.perform { try $0.aiProviderDelete(id: provider.id) }
                } catch let error as CoreError {
                    refusal = Refusal(error)
                } catch {
                    refusal = .storage
                }
            }
        case .onThisDevice, .ownKey:
            let draft = AiProviderDraft(
                id: existing.first?.id,
                name: engine == .onThisDevice ? "Local" : "My account",
                // The engine's own vocabulary. A local daemon and a hosted
                // account are different kinds, not one kind with two URLs.
                kind: engine == .onThisDevice ? "ollama" : "openai_compatible",
                // Empty means "the kind's own default endpoint", which for a
                // local daemon is the loopback address — not something to
                // restate here and let drift.
                baseUrl: baseUrl,
                // A provider without a model is not a provider. The engine
                // says so; this does not invent one on the reader's behalf.
                model: model.isEmpty ? (existing.first?.model ?? "") : model,
                timeoutMs: 0
            )
            let saved: AiProviderRow
            do {
                saved = try await store.perform { try $0.aiProviderSave(draft: draft) }
            } catch let error as CoreError {
                refusal = Refusal(error)
                return
            } catch {
                refusal = .storage
                return
            }
            if let apiKey, !apiKey.isEmpty {
                try? await store.perform {
                    try $0.aiApiKeySet(providerId: saved.id, key: apiKey)
                }
            } else if engine == .onThisDevice {
                // A local engine needs no key, and leaving a stale one behind
                // would keep a secret nothing uses.
                try? await store.perform { try $0.aiApiKeyClear(providerId: saved.id) }
            }
        }
        await load()
    }

    public func retrySync() async {
        guard !isRetrying else { return }
        isRetrying = true
        defer { isRetrying = false }
        _ = try? await store.perform { try $0.syncNow() }
        // A round can land rows, and what the extensions read is a document
        // somebody has to rewrite. The reader is looking at this screen, not
        // at the keyboard — which is exactly why the keyboard cannot notice
        // for itself.
        await SnapshotPublish.run(store: store)
        await load()
    }

    /// The numbers each chapter reports. Their words belong to the view, in
    /// the reader's language; what the model owes is the figures.
    public var deviceCount: Int { devices.count }

    private struct Snapshot: Sendable {
        let total: UInt32
        let vault: VaultFacts
        let enabled: Bool
        let queued: UInt64
        let lastSyncAt: Int64?
        let devices: [DeviceRow]
        let engine: AiEngine
    }
}

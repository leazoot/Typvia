// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// The vault room's state.
///
/// Unlocking, key derivation and decryption all happen behind the bridge. What
/// this decides is only *when to ask* and *what the room shows* — and while it
/// is shut, what it shows is nothing at all.
@MainActor
public final class VaultModel: ObservableObject {
    @Published public private(set) var phase: VaultPhase = .shut
    @Published public private(set) var entries: [VaultEntry] = []
    @Published public private(set) var clock: RelockClock?
    /// Set when a check did not recognise the reader. It carries no attempt
    /// count, because there is no attempt limit to report.
    @Published public private(set) var refusal: Refusal?

    private let store: TypviaStore
    private var tickTask: Task<Void, Never>?

    public init(store: TypviaStore) {
        self.store = store
    }

    public func load() async {
        let status = try? await store.perform { try $0.vaultStatus() }
        guard let status else {
            phase = .absent
            return
        }
        apply(status)
    }

    /// Opens with the platform's own gate — on iOS, reading the master key
    /// copy is itself the Face ID prompt, so there is no separate biometric
    /// plugin to trust.
    public func openWithBiometrics() async {
        await open(door: .faceId) { try $0.vaultUnlockBiometric() }
    }

    public func open(password: String) async {
        await open(door: .masterPassword) { try $0.vaultUnlockPassword(password: password) }
    }

    public func lock() async {
        tickTask?.cancel()
        let status = try? await store.perform { try $0.vaultLock() }
        entries = []
        clock = nil
        if let status { apply(status) } else { phase = .shut }
    }

    /// Anything that takes the screen away shuts the vault: leaving the room,
    /// the app going to the background, a screenshot. There is no transition —
    /// a vault that closes gracefully is a vault that is briefly still open.
    public func lockImmediately() {
        tickTask?.cancel()
        entries = []
        clock = nil
        phase = .shut
        Task { _ = try? await store.perform { try $0.vaultLock() } }
    }

    private func open(
        door: VaultDoor,
        _ unlock: @escaping @Sendable (TypviaCore) throws -> VaultStatus
    ) async {
        refusal = nil
        phase = .opening
        do {
            let status = try await store.perform(unlock)
            Haptic.success.play()
            apply(status)
            await readEntries()
            startTicking()
        } catch let error as CoreError {
            // A door that did not open is not a failure to report in red; it
            // is a state with a way out, and the vault is still shut, which is
            // the reassuring half. The door is kept because the kind of
            // refusal alone cannot say which one was tried.
            refusal = Refusal(error)
            phase = .didNotOpen(door)
            Haptic.light.play()
        } catch {
            refusal = .storage
            phase = .didNotOpen(door)
        }
    }

    private func apply(_ status: VaultStatus) {
        guard status.initialized else {
            phase = .absent
            return
        }
        guard status.unlocked else {
            phase = .shut
            clock = nil
            return
        }
        phase = .open
        clock = RelockClock(
            idleTimeoutMs: status.idleTimeoutMs,
            secondsLeft: Double(status.idleTimeoutMs) / 1000
        )
    }

    /// Names and timestamps only. The bridge returns no bodies for these rows
    /// and this asks for none.
    private func readEntries() async {
        let rows = try? await store.perform { core in
            try core.vaultList(limit: VaultEntry.pageLimit, offset: 0)
                .map(VaultEntry.init(snippet:))
        }
        entries = VaultOrder.sort(rows ?? [])
    }

    private func startTicking() {
        tickTask?.cancel()
        guard let clock else { return }
        tickTask = Task { [weak self] in
            var left = clock.secondsLeft
            while left > 0 {
                try? await Task.sleep(nanoseconds: 1_000_000_000)
                guard !Task.isCancelled else { return }
                left -= 1
                self?.clock = RelockClock(idleTimeoutMs: clock.idleTimeoutMs, secondsLeft: left)
            }
            await self?.lock()
        }
    }


}

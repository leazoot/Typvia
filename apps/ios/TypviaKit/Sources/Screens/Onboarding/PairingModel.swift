// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// Where the ciphertext this device is joining actually lives.
///
/// The two forms differ in what they need to be pointed at, and in nothing
/// else the reader can see: the key exchange, the check string and the check
/// are the same either way.
public enum PairingTarget: Equatable, Sendable {
    /// A Typvia sync server: its address and the account id, both readable on
    /// the device that already holds the library.
    case server
    /// A WebDAV folder: the same folder the other device uses. The account is
    /// read from the storage rather than typed.
    case webdav
}

/// Joining an account from this device.
///
/// This is the side that shows a code and waits. Nothing is installed until
/// the reader has compared the check string with the other screen and said they
/// match — which is the only thing standing between them and a server that
/// relays rather than delivers, so there is no way past it here.
@MainActor
public final class PairingModel: ObservableObject {
    /// Which numbered step is on screen. Nil while the reader is still saying
    /// where to join — that part is not one of the two steps, it is what has
    /// to be known before either of them means anything.
    @Published public private(set) var step: PairingStep?
    /// The code, and how long it is good for.
    @Published public private(set) var offered: PairingCode?
    /// The check string, arriving one character at a time.
    @Published public private(set) var reveal: SasReveal?
    /// The account's own fingerprint, as the claim reported it.
    @Published public private(set) var fingerprint: String?
    @Published public private(set) var refusal: Refusal?
    @Published public private(set) var isWorking = false
    /// Whether this device can hold a sync key at all. False on a host with no
    /// gated key storage — and then there is nothing to fill a form in for, so
    /// the screen says that instead of letting the reader type an address and
    /// discover it at the end.
    ///
    /// Nil while it has not been read, and **nil again if the read failed**:
    /// "this device cannot keep a key" is a claim about the device, not a way
    /// of saying "I did not find out". When it is unknown the screen makes no
    /// claim and lets the attempt produce the real answer.
    @Published public private(set) var canHoldAKey: Bool?

    @Published public var target: PairingTarget = .server
    @Published public var address = ""
    @Published public var accountId = ""
    @Published public var username = ""
    @Published public var password = ""
    /// This device's own master password, for the case where the offer carries
    /// vault material: the master key is re-wrapped under it and never travels
    /// in the clear.
    @Published public var masterPassword = ""

    private let store: TypviaStore
    private var pollTask: Task<Void, Never>?
    private var tickTask: Task<Void, Never>?

    /// How often the joining device asks whether the other one has answered.
    /// The same two seconds the desktop uses — the two products are waiting on
    /// the same server.
    static let pollInterval: TimeInterval = 2

    public init(store: TypviaStore) {
        self.store = store
    }

    public func load() async {
        let status = try? await store.perform { try $0.syncStatus() }
        canHoldAKey = status?.available
    }

    deinit {
        pollTask?.cancel()
        tickTask?.cancel()
    }

    /// Whether there is enough to ask with. Not a rule about addresses — the
    /// core decides what a usable address is — only that empty fields have
    /// nothing to send.
    public var canBegin: Bool {
        // Only a definite no stops the reader. Unknown lets them try, and the
        // attempt answers honestly — which is better than a screen refusing on
        // the strength of a question it never got an answer to.
        guard canHoldAKey != false else { return false }
        switch target {
        case .server:
            return !address.trimmed.isEmpty && !accountId.trimmed.isEmpty
        case .webdav:
            return !address.trimmed.isEmpty
        }
    }

    /// Starts the session and shows the code.
    @discardableResult
    public func begin() async -> Bool {
        guard canBegin, !isWorking else { return false }
        refusal = nil
        isWorking = true
        defer { isWorking = false }
        let target = target
        let address = address.trimmed
        let accountId = accountId.trimmed
        let credentials = webdavCredentials()
        do {
            let started = try await store.perform { core in
                switch target {
                case .server:
                    try core.pairingBegin(serverUrl: address, accountId: accountId)
                case .webdav:
                    try core.pairingBeginWebdav(baseUrl: address, credentials: credentials)
                }
            }
            // The typed secret is not kept once it has been handed over.
            password = ""
            // Not folded to zero when the transport promised no window: zero is
            // "expired", and the screen would declare a working code dead.
            offered = PairingCode(
                code: started.code,
                secondsLeft: started.expiresInSeconds.map(TimeInterval.init)
            )
            step = .offering(code: started.code, expiresAt: expiry(started.expiresInSeconds))
            startWaiting()
            startTicking(hasWindow: started.expiresInSeconds != nil)
            return true
        } catch let error as CoreError {
            refusal = Refusal(error)
        } catch {
            refusal = .storage
        }
        return false
    }

    /// The reader says the check string matches. This is the only path that
    /// installs anything.
    @discardableResult
    public func confirm() async -> Bool {
        guard !isWorking else { return false }
        refusal = nil
        isWorking = true
        defer { isWorking = false }
        let master = masterPassword
        do {
            _ = try await store.perform { core in
                try core.pairingFinalize(masterPassword: master.isEmpty ? nil : master)
            }
            masterPassword = ""
            stopWaiting()
            step = .joined
            Haptic.success.play()
            return true
        } catch let error as CoreError {
            refusal = Refusal(error)
        } catch {
            refusal = .storage
        }
        return false
    }

    /// The reader says they do not match, or leaves. Both drop the session on
    /// this device; nothing was installed either way.
    public func drop() {
        stopWaiting()
        offered = nil
        reveal = nil
        fingerprint = nil
        masterPassword = ""
        password = ""
        step = .dropped
        Task { [store] in _ = try? await store.perform { try $0.pairingCancel() } }
    }

    /// Back to the start, after a drop: the reader may simply have mistyped
    /// the address.
    public func startOver() {
        refusal = nil
        step = nil
        offered = nil
    }

    /// When the window runs out, in wall-clock terms — or nothing, when no
    /// window was promised.
    private func expiry(_ seconds: Int64?) -> Int64? {
        seconds.map { Int64(Date().timeIntervalSince1970 * 1000) + $0 * 1000 }
    }

    private func startWaiting() {
        pollTask?.cancel()
        pollTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(
                    nanoseconds: UInt64(PairingModel.pollInterval * 1_000_000_000)
                )
                guard !Task.isCancelled else { return }
                guard let self, await self.poll() else { continue }
                return
            }
        }
    }

    /// - Returns: whether the other device has answered.
    private func poll() async -> Bool {
        // A failed poll is the other device not having answered yet, or the
        // network being away. Neither is news: the screen keeps waiting and
        // keeps saying that nothing has been installed.
        let answered = try? await store.perform { try $0.pairingPoll() }
        guard let claim = answered ?? nil else { return false }
        fingerprint = claim.rootFingerprint
        step = .comparing(sas: claim.sas, otherDevice: claim.rootFingerprint)
        revealSas(claim.sas)
        return true
    }

    /// The characters arrive one at a time. Arriving at once invites a glance;
    /// the reader is being asked to compare, which takes reading.
    private func revealSas(_ sas: String) {
        reveal = SasReveal(sas: sas, shown: 0)
        tickTask?.cancel()
        tickTask = Task { [weak self] in
            // The separators are not characters anybody compares, so they are
            // not beats either: the count comes from the reveal, not from the
            // string.
            let count = SasReveal(sas: sas, shown: 0).characters.count
            let step = SasReveal.interval(for: count)
            for shown in 1...max(count, 1) {
                try? await Task.sleep(nanoseconds: UInt64(step * 1_000_000_000))
                guard !Task.isCancelled else { return }
                self?.reveal = SasReveal(sas: sas, shown: shown)
            }
        }
    }

    /// Counts the window down, when there is a window. A code with no promised
    /// window is not counted down to zero — that would state an expiry nobody
    /// undertook.
    private func startTicking(hasWindow: Bool) {
        tickTask?.cancel()
        guard hasWindow else { return }
        tickTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 1_000_000_000)
                guard !Task.isCancelled, let self, let offered = self.offered else { return }
                guard let left = offered.secondsLeft, left > 0 else { return }
                self.offered = PairingCode(code: offered.code, secondsLeft: left - 1)
            }
        }
    }

    private func stopWaiting() {
        pollTask?.cancel()
        pollTask = nil
        tickTask?.cancel()
        tickTask = nil
    }

    /// The credentials, packed by the sync crate rather than by this screen —
    /// the string's shape is the transport's, not the interface's.
    private func webdavCredentials() -> String? {
        guard target == .webdav else { return nil }
        return TypviaKit.webdavCredentials(username: username.trimmed, password: password)
    }
}

extension String {
    var trimmed: String { trimmingCharacters(in: .whitespacesAndNewlines) }
}

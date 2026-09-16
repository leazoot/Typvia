// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// Settings is a contents page.
///
/// Six numbered chapters, each of which is a whole page rather than a folding
/// panel. The number is both a layout element and the navigation; there are no
/// icons anywhere in this room, and no switches at this level — a chapter's
/// current value is stated in mono beside its title, which is what a reader is
/// scanning for.
public enum SettingsChapter: Int, CaseIterable, Identifiable, Sendable {
    case sync = 1
    case keyboard
    case ai
    case vault
    case appearance
    case data

    public var id: Int { rawValue }

    /// The two-digit number the page is set with.
    public var number: String { String(format: "%02d", rawValue) }
}

/// One device this key recognises.
public struct DeviceRow: Identifiable, Equatable, Sendable {
    /// What the device is doing, in one word. Never a colour on its own.
    public enum State: Equatable, Sendable {
        case inUse
        /// Changes are waiting to go to this device.
        case waiting(UInt64)
        case lastSeen(Int64?)
    }

    public let id: String
    public let name: String
    public let isThisDevice: Bool
    public let state: State

    init(device: SyncDevice, pending: UInt64) {
        id = device.deviceId
        name = device.name
        isThisDevice = device.isThisDevice
        if device.isThisDevice {
            state = pending > 0 ? .waiting(pending) : .inUse
        } else {
            state = .lastSeen(device.revokedAt == nil ? device.createdAt : nil)
        }
    }
}

/// Which machine an AI action's text would be handed to.
///
/// The choice is the whole point of the chapter: it decides how far a selected
/// piece of text travels. Whichever is chosen, a snippet from the vault is
/// never sent to any model.
public enum AiEngine: String, CaseIterable, Equatable, Sendable {
    case onThisDevice
    case ownKey
    case off

    /// Read from what is actually configured rather than from a preference
    /// of its own: a settings screen that keeps its own copy of the truth
    /// eventually disagrees with it.
    public static func inEffect(providers: [AiProviderRow]) -> AiEngine {
        guard let provider = providers.first else { return .off }
        return AiEngine.isLocal(provider.baseUrl) ? .onThisDevice : .ownKey
    }

    /// A base URL that never leaves the machine.
    static func isLocal(_ baseUrl: String) -> Bool {
        guard let host = URL(string: baseUrl)?.host?.lowercased() else { return false }
        return host == "localhost" || host == "127.0.0.1" || host == "::1"
    }
}

/// What the sync chapter says when rounds keep failing.
///
/// Same shape as the home screen's notice, and for the same reason: what is
/// still true first, then what happened, then one thing to do. Clay, never
/// red, and never a dialog.
public struct SyncTrouble: Equatable, Sendable {
    public let queued: UInt64
    public let lastSyncAt: Int64?
    public let total: UInt32
}

/// The data chapter's headline figure.
public struct DataStats: Equatable, Sendable {
    public let total: UInt32
    public let sorts: Int
}

/// What the vault chapter is allowed to know from outside the vault.
///
/// Whether there is a vault, and how long it stays open. Not what is in it,
/// not how much — the chapter is a page about the room, and it is read while
/// the room is shut.
public struct VaultFacts: Equatable, Sendable {
    public let isMade: Bool
    public let idleTimeoutMs: Int64

    public init(isMade: Bool, idleTimeoutMs: Int64) {
        self.isMade = isMade
        self.idleTimeoutMs = idleTimeoutMs
    }
}

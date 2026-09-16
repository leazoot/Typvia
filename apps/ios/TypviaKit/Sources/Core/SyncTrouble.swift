// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// What a reader can do about a stopped sync round.
///
/// The point of splitting this out is the one case that used to be missing:
/// some failures cannot be retried into working. Offering "try again" against
/// a rejected session, a pinned certificate or a protocol gap is a button that
/// is guaranteed to fail, and a button that always fails teaches the reader to
/// stop reading the card.
public enum SyncRemedy: Equatable, Sendable {
    /// Running the round again could plausibly end differently.
    case tryAgain
    /// It needs a decision the settings room holds — an address, an account,
    /// a device to re-verify.
    case openSettings
    /// Nothing to press. The engine is already waiting and will go on its own.
    case waitItOut
}

extension SyncFailureKind {
    /// The cause clause, in the reader's language.
    ///
    /// A category is all that crosses the bridge, and a category is all that is
    /// worded here. There is no branch for "and the server said …" because the
    /// server's own words never arrive — they can name the host, the account
    /// or the path, and none of that helps the reader decide what to do next.
    public func cause(_ tr: Translator) -> String {
        switch self {
        case .unreachable:
            return tr("The server could not be reached.", "连不上服务器。")
        case .serverAddress:
            return tr("The server address cannot be used.", "服务器地址用不了。")
        case .auth:
            return tr("The server would not accept this device.", "服务器不认这台设备。")
        case .protocolVersion:
            return tr(
                "This version and the server do not speak the same protocol.",
                "这个版本和服务器说的协议对不上。"
            )
        case .busy:
            return tr("The server asked for a pause.", "服务器让先缓一缓。")
        case .serverRefused:
            return tr("The server turned the last round down.", "服务器回绝了上一轮。")
        case .serverUnexpected:
            return tr(
                "The server's reply was not one this version can read.",
                "服务器的答复,这个版本读不懂。"
            )
        case .trust:
            return tr("A device certificate did not check out.", "有一台设备的证书没验过。")
        case .notSetUp:
            return tr("Sync is not set up on this device.", "这台设备还没设置同步。")
        case .thisDevice:
            return tr("Something on this device stopped the round.", "是这台设备上的事挡住了这一轮。")
        }
    }

    /// What the card should offer.
    public var remedy: SyncRemedy {
        switch self {
        case .unreachable, .serverRefused, .serverUnexpected, .thisDevice:
            return .tryAgain
        case .serverAddress, .auth, .protocolVersion, .trust, .notSetUp:
            return .openSettings
        case .busy:
            return .waitItOut
        }
    }
}

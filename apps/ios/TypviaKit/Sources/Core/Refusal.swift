// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// Why something did not happen.
///
/// The engine's own sentences are written in English, and they are written for
/// whoever is reading a log. Quoting one into the interface breaks the rule
/// this product does not bend — one screen, one language — and hands a Chinese
/// reader an English sentence at the exact moment they most need to understand
/// it. So what crosses is the kind of refusal, and the words are written here,
/// in both languages, beside the screens that show them.
///
/// What is lost by not quoting is precision, and it is not lost quietly: a
/// screen that knows which rule it just risked breaking should say so itself,
/// because it knows what the reader was doing and the engine does not.
public enum Refusal: Equatable, Sendable {
    /// The input can be corrected.
    case invalid
    /// A rule was broken: something is already taken, still referenced, or in
    /// a state that does not allow this.
    case clash
    /// It needs a permission the app has not been given, or a vault that is
    /// shut.
    case notPermitted
    /// An app rule blocks this for that destination.
    case ruleBlocked
    /// A remote service could not be reached. Not a fault to report — the
    /// product goes on working locally.
    case offline
    /// The record addressed is not there.
    case missing
    /// Storage failed. Never quoted: those messages can carry paths and SQL.
    case storage

    public init(_ error: CoreError) {
        switch error {
        case .Validation: self = .invalid
        case .Conflict: self = .clash
        case .PermissionDenied: self = .notPermitted
        case .RuleBlocked: self = .ruleBlocked
        case .Unavailable: self = .offline
        case .NotFound: self = .missing
        case .System: self = .storage
        }
    }

    /// The sentence a screen shows when it has nothing more specific to say.
    ///
    /// Each one is written the way this product writes failures: what the
    /// state actually is, not what went wrong internally, and never an
    /// instruction to try again for the kinds where trying again is the same
    /// call with the same answer.
    public func sentence(_ tr: Translator) -> String {
        switch self {
        case .invalid:
            return tr("Something in this is not usable yet.", "这里面有一处还不能用。")
        case .clash:
            return tr("Something here is already taken.", "这里有一处已经被占用了。")
        case .notPermitted:
            return tr("This needs a permission it does not have.", "这件事需要一项它还没有的权限。")
        case .ruleBlocked:
            return tr("A rule you set blocks this here.", "你定的一条规则在这里挡住了它。")
        case .offline:
            return tr("That could not be reached just now.", "刚才够不着那一端。")
        case .missing:
            return tr("That is no longer here.", "它已经不在了。")
        case .storage:
            return tr("It could not be written just now.", "刚才没写进去。")
        }
    }
}

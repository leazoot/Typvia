// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// What the AI is allowed to be asked, from this screen.
///
/// Two things, and both of them hand text to a model. So the rule that matters
/// most here is the one that is not enforced here at all: a snippet from the
/// vault is refused by the shared layer *before* a prompt is built. This
/// passes the flag through and never decides it — a screen that decided what
/// counted as sensitive would be a second opinion on the only question where
/// there must be one.

/// Where the AI strip is.
public enum AiStripPhase: Equatable, Sendable {
    /// Nothing asked. The strip is one word.
    case idle
    /// A round trip is out. The caret sweeps; there is no spinner in this
    /// product.
    case working
    /// A suggestion came back and has not been taken or dismissed.
    case suggested(AiSuggestion)
    /// An action's output, waiting to replace the body or be dropped.
    case produced(text: String, maskedKinds: [String])
    /// It did not happen. The sentence is the engine's.
    case refused(Refusal)
    /// No engine is configured. Not a failure — a thing not set up, and the
    /// strip says where to set it up rather than reporting an error.
    case noEngine
}

/// What the model proposed for a draft. Nothing is applied until the reader
/// says so — the strip proposes, the reader disposes.
public struct AiSuggestion: Equatable, Sendable {
    public let title: String?
    public let snippetType: String?
    public let trigger: String?
    public let tags: [String]

    init(_ suggestion: AiOrganizeSuggestion) {
        title = suggestion.title
        snippetType = suggestion.snippetType
        trigger = suggestion.trigger
        tags = suggestion.tags.map(\.name)
    }

    /// Whether there is anything worth showing. An answer that proposes
    /// nothing is not a suggestion, and a strip that lights up for it teaches
    /// the reader to ignore it.
    public var isEmpty: Bool {
        title == nil && snippetType == nil && trigger == nil && tags.isEmpty
    }
}

/// One saved action, as the strip lists it.
public struct AiActionRow: Identifiable, Equatable, Sendable {
    public let id: String
    public let name: String
    /// Where the action's text is allowed to come from. It is a contract, not
    /// a hint: running one with text from anywhere else is refused.
    public let inputSource: String

    init(action: AiAction) {
        id = action.id
        name = action.name
        inputSource = action.inputSource
    }

    /// The source this screen supplies. A snippet's own body — the strip
    /// cannot offer an action whose contract asks for something else.
    public static let source = "snippet"

    public var canRunHere: Bool { inputSource == AiActionRow.source }
}

/// The sentences the strip says when nothing came back.
///
/// The kinds are stated one to one rather than falling through to the shared
/// sentences, because two of the shared ones are wrong here. A broken rule is
/// not "something is already taken": on this path it is the egress gate
/// refusing to send, a missing key, or the provider itself saying no — the
/// core folds all three into one kind, so this sentence claims nothing about
/// whether anything left the device. And a refused permission is not the vault
/// gate; this strip is not composed for a secret at all.
public enum AiStripCopy {
    public static func refusal(_ refusal: Refusal, _ tr: Translator) -> String {
        switch refusal {
        case .invalid:
            tr("The engine would not take that as it is.", "引擎不接受这样的内容。")
        case .clash:
            tr("Nothing came back, and nothing here changed.", "没有拿到结果,这里也什么都没变。")
        case .offline:
            tr(
                "The engine could not be reached. Everything here is as it was.",
                "没能连上引擎。这里的东西一如原样。"
            )
        case .notPermitted, .ruleBlocked:
            tr("This needs something it has not been given.", "这件事需要一项它还没有的东西。")
        case .missing:
            tr("That action is no longer here.", "这个动作已经不在了。")
        case .storage:
            tr("It did not go through, and nothing here changed.", "没走通,这里也什么都没变。")
        }
    }
}

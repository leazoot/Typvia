// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// Filing what the share sheet left in the inbox.
///
/// The extension wrote down what the reader chose; the rules are the core's,
/// and this is where they get applied. Two things must hold whatever happens:
/// nothing the reader shared is lost, and the inbox is only cleared for the
/// items that actually landed.
public enum InboxIntake {
    /// What became of one shared item.
    public enum Outcome: Equatable, Sendable {
        case filed(id: String)
        /// The trigger was already taken, so the snippet was filed without
        /// one. Losing the text over a name collision would be the wrong
        /// trade — the trigger can be added later, the text cannot be
        /// recovered from a cleared inbox.
        case filedWithoutTrigger(id: String)
        /// Could not be filed at all; it stays in the inbox for next time.
        case kept
    }

    /// Files everything waiting, and clears only what landed.
    ///
    /// - Returns: what happened to each item, in the order they were shared.
    @discardableResult
    public static func run(store: TypviaStore) async -> [Outcome] {
        let waiting = Inbox.read()
        guard !waiting.isEmpty else { return [] }

        var outcomes: [Outcome] = []
        var unfiled: [InboxItem] = []
        for item in waiting {
            let outcome = await file(item, store: store)
            outcomes.append(outcome)
            if outcome == .kept { unfiled.append(item) }
        }

        // Only what did not land stays. Clearing the lot on a partial failure
        // is how a reader loses something they watched the sheet accept.
        Inbox.clear()
        for item in unfiled {
            Inbox.append(item)
        }
        return outcomes
    }

    private static func file(_ item: InboxItem, store: TypviaStore) async -> Outcome {
        let draft = SnippetDraft(
            title: item.title,
            body: item.body,
            snippetType: item.snippetType,
            description: nil,
            folderId: nil,
            trigger: item.trigger,
            // The default belongs to the shared layer; the intake states the
            // trigger and lets it decide what kind of trigger that is.
            triggerMode: nil,
            language: nil
        )
        do {
            let saved = try await store.perform { try $0.snippetCreate(draft: draft) }
            return .filed(id: saved.id)
        } catch CoreError.Conflict {
            // A taken trigger is the one refusal worth retrying, because the
            // text is fine and only its shorthand collided.
            guard item.trigger != nil else { return .kept }
            let withoutTrigger = SnippetDraft(
                title: draft.title,
                body: draft.body,
                snippetType: draft.snippetType,
                description: nil,
                folderId: nil,
                trigger: nil,
                triggerMode: nil,
                language: nil
            )
            let saved = try? await store.perform { try $0.snippetCreate(draft: withoutTrigger) }
            return saved.map { .filedWithoutTrigger(id: $0.id) } ?? .kept
        } catch {
            return .kept
        }
    }
}

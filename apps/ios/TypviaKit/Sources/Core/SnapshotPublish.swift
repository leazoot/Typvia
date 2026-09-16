// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// Putting the snapshot where the extensions can read it.
///
/// The keyboard and the widget have no database — one process owns the single
/// writer and it is the app, so what the app does not publish, they cannot
/// know. An unpublished snapshot does not show them a stale library; it shows
/// them an empty one, which reads on a home screen as a product with nothing
/// in it.
///
/// The document itself is generated behind the bridge: which snippets travel,
/// what a secret is allowed to carry, and how a body is withheld are the
/// core's answers, and none of them are restated here.
public enum SnapshotPublish {
    /// Rewrites the snapshot in the shared container.
    ///
    /// - Returns: whether a new document was written. Nothing is surfaced on
    ///   failure: the write is a temp file plus a rename, so the extensions
    ///   keep reading the previous document, and there is no screen where a
    ///   failed republish is something the reader can act on.
    @discardableResult
    public static func run(store: TypviaStore) async -> Bool {
        guard let directory = SharedContainer.directory else { return false }
        return await write(store: store, to: directory)
    }

    @discardableResult
    public static func write(store: TypviaStore, to directory: URL) async -> Bool {
        let done: Void? = try? await store.perform { core in
            try core.writeSnapshot(directory: directory.path)
        }
        return done != nil
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// The app's one handle on its data.
///
/// Everything this type does is about *when* and *where* a call runs, never
/// about what it means. The rules — what a valid trigger is, when a version is
/// recorded, what gets indexed, in which order a secret is encrypted — all
/// live behind the bridge, and restating any of them here would give the
/// product two answers to the same question.
///
/// Being an actor buys two things the storage rules require. Calls serialise,
/// so the single writer stays single; and they run on the cooperative pool
/// rather than the main thread, so a query never stalls a frame.
///
/// One instance per process. A second one over the same directory would put
/// two writers on one database.
public actor TypviaStore {
    private let core: TypviaCore

    /// Opens the database under `dataDirectory`, creating it on first run and
    /// migrating it to the current schema. A migration that cannot complete
    /// leaves the file as it was and throws.
    public init(dataDirectory: URL) throws {
        core = try TypviaCore.open(dataDir: dataDirectory.path)
    }

    /// Runs one call against the core.
    ///
    /// The bridge is already a typed surface, so this hands it over as-is
    /// instead of restating thirty signatures that would drift from it. What
    /// the caller gains by coming through here is the actor: serialised, off
    /// the main thread.
    public func perform<T: Sendable>(
        _ body: @Sendable (TypviaCore) throws -> T
    ) throws -> T {
        try body(core)
    }
}

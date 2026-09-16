// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// The one place the app and its extensions meet.
///
/// Two channels, both one-way. The app writes a snapshot the extensions read,
/// and the share sheet writes an inbox the app ingests. No extension opens the
/// database: one process owns the single writer, and that process is the app.
public enum SharedContainer {
    public static let appGroup = "group.dev.typvia.mobile"

    public static var directory: URL? {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroup)
    }

    /// What the extensions read. Written by the app.
    public static var snapshotURL: URL? {
        directory?.appendingPathComponent("snapshot.json")
    }

    /// What the share sheet writes. Read and cleared by the app.
    public static var inboxURL: URL? {
        directory?.appendingPathComponent("inbox.json")
    }

    /// Reads the snapshot, or nothing.
    ///
    /// Nothing is not an error worth reporting from an extension: a widget
    /// that has never been fed shows an empty face, which is the truth.
    public static func snapshotJSON() -> String? {
        guard let snapshotURL, let data = try? Data(contentsOf: snapshotURL) else { return nil }
        return String(data: data, encoding: .utf8)
    }
}

/// One thing the share sheet handed over, waiting for the app to file it.
///
/// It is a plain record on purpose: the extension does not decide anything the
/// core decides — not the kind, not whether the trigger collides, not whether
/// the text is sensitive. It writes down what the reader chose and lets the
/// app ask the core.
public struct InboxItem: Codable, Equatable, Sendable {
    public let title: String
    public let body: String
    public let trigger: String?
    public let snippetType: String
    public let sharedAt: Int64

    public init(title: String, body: String, trigger: String?, snippetType: String, sharedAt: Int64) {
        self.title = title
        self.body = body
        self.trigger = trigger
        self.snippetType = snippetType
        self.sharedAt = sharedAt
    }
}

/// The inbox: a list, because a reader can share twice before opening the app.
public enum Inbox {
    /// Appends one item. Reading the existing list first means a second share
    /// does not overwrite the first — the failure nobody notices until they
    /// have lost something.
    @discardableResult
    public static func append(_ item: InboxItem) -> Bool {
        guard let url = SharedContainer.inboxURL else { return false }
        var items = read()
        items.append(item)
        guard let data = try? JSONEncoder().encode(items) else { return false }
        do {
            // Atomic: a share sheet that is killed mid-write must not leave a
            // half-written file that the app then fails to parse and drops.
            try data.write(to: url, options: .atomic)
            return true
        } catch {
            return false
        }
    }

    public static func read() -> [InboxItem] {
        guard let url = SharedContainer.inboxURL,
              let data = try? Data(contentsOf: url),
              let items = try? JSONDecoder().decode([InboxItem].self, from: data)
        else { return [] }
        return items
    }

    /// Clears the inbox once its contents are safely in the database. Called
    /// by the app, never by the extension.
    public static func clear() {
        guard let url = SharedContainer.inboxURL else { return }
        try? FileManager.default.removeItem(at: url)
    }
}

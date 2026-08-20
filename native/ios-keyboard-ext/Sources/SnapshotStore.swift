// Read-only access to the KeyboardSnapshot document the host app writes
// into the App Group container. The extension NEVER writes to the container;
// parsing and validation live in Rust (typvia-mobile-ffi) so a malformed,
// stale, or newer-version file degrades to the empty state instead of
// crashing (KeyboardSnapshot validate() rejects unknown versions).

import Foundation

struct SnapshotStore {
    /// Must match the `com.apple.security.application-groups` entitlement of
    /// both the host app and this extension (apps/mobile app_group.rs).
    static let appGroupId = "group.dev.typvia.mobile"
    /// File name the host writes atomically (snapshot_file.rs).
    static let snapshotFileName = "snapshot.json"

    /// The raw document, kept for follow-up FFI calls (search, body lookup).
    let json: String
    /// Entries in default display order (recent first, then file order).
    let entries: [SnapshotEntry]
    /// Folders holding at least one entry, in display order.
    let folders: [SnapshotFolder]

    /// Loads and validates the current snapshot. `nil` covers every
    /// degraded case the same way — no container (entitlement missing),
    /// no file yet, unreadable bytes, failed validation — because the
    /// keyboard's answer to all of them is the same honest empty state.
    /// Errors are deliberately not logged: snapshot-adjacent messages could
    /// reach system logs.
    static func load() -> SnapshotStore? {
        guard
            let container = FileManager.default.containerURL(
                forSecurityApplicationGroupIdentifier: appGroupId)
        else { return nil }
        let url = container.appendingPathComponent(snapshotFileName)
        guard
            let data = try? Data(contentsOf: url),
            let json = String(data: data, encoding: .utf8),
            let entries = try? snapshotEntries(json: json),
            let folders = try? snapshotFolders(json: json)
        else { return nil }
        return SnapshotStore(json: json, entries: entries, folders: folders)
    }

    /// Entries matching a search query (Rust-side, case-insensitive over
    /// title/trigger/body of normal entries). Empty query returns the
    /// default list. A read that fails mid-session degrades to no rows.
    func search(_ query: String) -> [SnapshotEntry] {
        (try? filterEntries(json: json, query: query)) ?? []
    }

    /// Body text for insertion; `nil` for sensitive or unknown ids, so a
    /// locked row can never insert plaintext.
    func body(for id: String) -> String? {
        (try? entryBody(json: json, id: id)) ?? nil
    }

    /// Ordered `{{variable}}` names of one entry's body, deduplicated by
    /// first occurrence. Empty for plain bodies; the FFI rejects sensitive
    /// and unknown ids, which degrade to empty here (no fill face opens).
    func variableNames(for id: String) -> [String] {
        (try? templateVariables(json: json, id: id)) ?? []
    }

    /// Lenient fill preview: provided values substituted, missing variables
    /// shown as `‹name›` placeholders. `nil` on any FFI rejection.
    func fillPreview(id: String, values: [String: String]) -> String? {
        try? templateFillPreview(json: json, id: id, values: values)
    }

    /// Final insert text: provided values substituted, missing variables
    /// rendered empty. `nil` on any FFI rejection.
    func fillRender(id: String, values: [String: String]) -> String? {
        try? templateFillRender(json: json, id: id, values: values)
    }
}

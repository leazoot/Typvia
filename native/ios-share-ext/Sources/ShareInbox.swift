// Write-only inbox channel into the App Group container: one JSON document
// per share, landed tmp+rename so the main app never observes a half-written
// file. The extension never reads the database, the keyboard snapshot, or
// anything else in the container — write-only by contract, because extensions
// never touch the main store.

import Foundation

/// Failures are reported without any shared content (log red line); the two
/// cases only distinguish "this device has no container" from "the write
/// itself failed".
enum ShareInboxError: Error {
    case noContainer
    case writeFailed
}

/// Inbox document, schema v1 (pinned; the host's share_inbox.rs is the
/// reading counterpart): `{ "schema_version": 1, "shared_at": <unix ms>,
/// "title"?: string, "source_hint"?: string, "text": string }`. iOS exposes
/// no supported way for a share extension to learn the host app's name, so
/// `source_hint` stays reserved and is never written here.
struct ShareInboxDocument: Encodable {
    let schemaVersion: Int
    let sharedAt: Int64
    let title: String?
    let text: String

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case sharedAt = "shared_at"
        case title
        case text
    }

    init(sharedAt: Int64, title: String?, text: String) {
        self.schemaVersion = 1
        self.sharedAt = sharedAt
        self.title = title
        self.text = text
    }
}

enum ShareInbox {
    /// Must match the App Group entitlement of this target and the host app.
    static let appGroupId = "group.dev.typvia.mobile"
    static let inboxDirName = "inbox"
    /// Mirror of the host's SHARE_TEXT_MAX_BYTES (UTF-8 bytes); Save is
    /// disabled beyond it so the host never has to quarantine our writes.
    static let textMaxBytes = 128 * 1024
    /// Mirror of the host/editor derived-title bound.
    static let titleMaxChars = 60

    /// The derived-title preview shown as the Title placeholder: first
    /// non-empty line, bounded. The host applies the same rule when no title
    /// is sent, so leaving the field empty and this preview agree.
    static func derivedTitle(from text: String) -> String {
        let line = text
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .first { !$0.isEmpty } ?? ""
        return String(line.prefix(titleMaxChars))
    }

    /// Atomically lands one document as `inbox/share-<uuid>.json`: encode,
    /// write `<name>.tmp` in the same directory, rename. A failure at any
    /// point leaves no readable partial file (`*.json.tmp` is outside the
    /// host's scan set) and the tmp file is cleaned up best-effort.
    static func write(_ document: ShareInboxDocument, fileManager: FileManager = .default) throws {
        guard
            let container = fileManager.containerURL(
                forSecurityApplicationGroupIdentifier: appGroupId)
        else {
            throw ShareInboxError.noContainer
        }
        let inbox = container.appendingPathComponent(inboxDirName, isDirectory: true)
        let name = "share-\(UUID().uuidString.lowercased()).json"
        let finalURL = inbox.appendingPathComponent(name)
        let tmpURL = inbox.appendingPathComponent(name + ".tmp")
        do {
            try fileManager.createDirectory(at: inbox, withIntermediateDirectories: true)
            let data = try JSONEncoder().encode(document)
            try data.write(to: tmpURL)
            try fileManager.moveItem(at: tmpURL, to: finalURL)
        } catch {
            try? fileManager.removeItem(at: tmpURL)
            throw ShareInboxError.writeFailed
        }
    }
}

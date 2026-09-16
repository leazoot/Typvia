// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// The document the keyboard and the widget read is only as good as the app's
/// habit of writing it. These cases stand where the two processes meet: what
/// the reader can reach through an extension, and what must never travel there
/// no matter who asks.
final class SnapshotPublishTests: XCTestCase {
    private var directory: URL!
    private var shared: URL!
    private var store: TypviaStore!

    override func setUpWithError() throws {
        let root = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
        directory = root.appendingPathComponent("data")
        shared = root.appendingPathComponent("shared")
        try FileManager.default.createDirectory(at: shared, withIntermediateDirectories: true)
        store = try TypviaStore(dataDirectory: directory)
    }

    override func tearDownWithError() throws {
        store = nil
        try? FileManager.default.removeItem(at: directory)
        try? FileManager.default.removeItem(at: shared)
    }

    private func draft(_ title: String, _ body: String, trigger: String?) -> SnippetDraft {
        SnippetDraft(
            title: title,
            body: body,
            snippetType: "command",
            description: nil,
            folderId: nil,
            trigger: trigger,
            triggerMode: trigger.map { _ in "delimiter" },
            language: nil
        )
    }

    private func published() throws -> String {
        let url = shared.appendingPathComponent("snapshot.json")
        return try String(contentsOf: url, encoding: .utf8)
    }

    func testWhatTheAppPublishesIsWhatTheExtensionsCanRead() async throws {
        _ = try await store.perform {
            try $0.snippetCreate(draft: draft("Tail logs", "docker logs -f app", trigger: ";dlog"))
        }
        _ = try await store.perform {
            try $0.snippetCreate(draft: draft("Cluster login", "kubectl config", trigger: ";k8s"))
        }

        let written = await SnapshotPublish.write(store: store, to: shared)
        XCTAssertTrue(written)

        let json = try published()
        XCTAssertEqual(try parseSnapshot(json: json).entryTotal, 2)
        // The widget asks for one row and reports the library's size beside it;
        // both numbers come from this one document.
        XCTAssertEqual(try widgetEntries(json: json, limit: 1).count, 1)
    }

    func testAnUnpublishedLibraryIsAnEmptyOneRatherThanAStaleOne() async throws {
        _ = try await store.perform {
            try $0.snippetCreate(draft: draft("Tail logs", "docker logs -f app", trigger: ";dlog"))
        }
        // Nothing was published, so there is nothing to read — the state the
        // widget was left in for as long as no one wrote this file.
        XCTAssertNil(try? published())
    }

    func testAFailedPublishSaysSoAndLeavesTheLastDocumentAlone() async throws {
        _ = try await store.perform {
            try $0.snippetCreate(draft: draft("Tail logs", "docker logs -f app", trigger: ";dlog"))
        }
        let first = await SnapshotPublish.write(store: store, to: shared)
        XCTAssertTrue(first)
        let before = try published()

        let missing = shared.appendingPathComponent("not-a-directory")
        let second = await SnapshotPublish.write(store: store, to: missing)
        XCTAssertFalse(second)
        XCTAssertEqual(try published(), before)
    }

    func testASecretBodyIsNotInTheFileTheExtensionsCanOpen() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: "correct horse battery") }
        _ = try await store.perform {
            try $0.vaultCreateSecret(
                draft: SnippetDraft(
                    title: "Deploy key",
                    body: "AKIA_FAKE_NOT_A_SECRET_deploy_value",
                    snippetType: "sensitive",
                    description: nil,
                    folderId: nil,
                    trigger: nil,
                    triggerMode: nil,
                    language: nil
                )
            )
        }

        let written = await SnapshotPublish.write(store: store, to: shared)
        XCTAssertTrue(written)
        // Byte level, not field level: this file lands in a container every
        // extension on the device can open, so what matters is that the string
        // is not in it — whatever shape the document takes.
        XCTAssertFalse(try published().contains("AKIA_FAKE_NOT_A_SECRET_deploy_value"))
    }
}

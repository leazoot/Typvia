// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// Proves the bridge is reachable and complete enough from Swift: a snippet
/// makes the whole round trip — create, read, edit, search, recycle bin,
/// restore — through the generated bindings and a real database on disk.
/// The behaviour itself is covered by the Rust tests; what is under test here
/// is that the boundary carries it faithfully.
final class TypviaStoreTests: XCTestCase {
    private var directory: URL!
    private var store: TypviaStore!

    override func setUpWithError() throws {
        directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
        store = try TypviaStore(dataDirectory: directory)
    }

    override func tearDownWithError() throws {
        store = nil
        try? FileManager.default.removeItem(at: directory)
    }

    private func draft(
        _ title: String,
        _ body: String,
        trigger: String? = nil
    ) -> SnippetDraft {
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

    func testASnippetMakesTheFullRoundTripThroughTheBridge() async throws {
        let created = try await store.perform {
            try $0.snippetCreate(draft: draft("Tail logs", "docker logs -f app", trigger: ";dlog"))
        }
        XCTAssertFalse(created.id.isEmpty)
        XCTAssertEqual(created.version, 1)
        XCTAssertEqual(created.body, "docker logs -f app")

        let listed = try await store.perform {
            try $0.snippetListPage(view: "all", folderId: nil, snippetType: nil, limit: 20, offset: 0)
        }
        XCTAssertEqual(listed.map(\.id), [created.id])

        let edited = try await store.perform { core -> Snippet in
            var edit = SnippetEdit(
                id: created.id,
                title: created.title,
                body: "docker logs -f web",
                snippetType: created.snippetType,
                description: created.description,
                folderId: created.folderId,
                trigger: created.trigger,
                triggerMode: created.triggerMode,
                language: created.language,
                isFavorite: true,
                isPinned: created.isPinned,
                isEnabled: created.isEnabled
            )
            edit.title = "Tail web logs"
            return try core.snippetUpdate(edit: edit)
        }
        XCTAssertEqual(edited.version, 2)
        XCTAssertTrue(edited.isFavorite)

        let hits = try await store.perform {
            try $0.searchSnippets(query: "Tail web logs", limit: 20, offset: 0)
        }
        XCTAssertEqual(hits.map(\.snippetId), [created.id])
        XCTAssertEqual(hits.first?.tier, "title_exact")

        try await store.perform { try $0.snippetTrash(id: created.id) }
        let afterTrash = try await store.perform { try $0.libraryCounts() }
        XCTAssertEqual(afterTrash.total, 0)
        XCTAssertEqual(afterTrash.trash, 1)

        try await store.perform { try $0.snippetRestore(id: created.id) }
        let afterRestore = try await store.perform { try $0.libraryCounts() }
        XCTAssertEqual(afterRestore.total, 1)
        XCTAssertEqual(afterRestore.trash, 0)
    }

    /// A rule violation arrives as its own case, so a screen can tell "you can
    /// fix this" from "something broke" without reading the message.
    func testADuplicateTriggerArrivesAsATypedConflict() async throws {
        _ = try await store.perform {
            try $0.snippetCreate(draft: draft("Tail logs", "docker logs", trigger: ";dlog"))
        }

        do {
            _ = try await store.perform {
                try $0.snippetCreate(draft: draft("Pod logs", "kubectl logs", trigger: ";dlog"))
            }
            XCTFail("a duplicate trigger must not be accepted")
        } catch let error as CoreError {
            guard case .Conflict = error else {
                return XCTFail("expected a conflict, got \(error)")
            }
        }
    }

    func testAnUnknownIdArrivesAsNotFound() async throws {
        do {
            _ = try await store.perform { try $0.snippetGet(id: "no-such-id") }
            XCTFail("an unknown id must not resolve")
        } catch let error as CoreError {
            guard case .NotFound = error else {
                return XCTFail("expected not found, got \(error)")
            }
        }
    }

    /// Red line: an error crossing the bridge names the rule, never the text
    /// that broke it. The fixture body is a deliberate fake so the assertion
    /// can grep the message for it.
    func testAnErrorMessageNeverCarriesSnippetContent() async throws {
        _ = try await store.perform {
            try $0.snippetCreate(draft: draft("Key", "AKIAFAKEEXAMPLE00000", trigger: ";key"))
        }

        do {
            _ = try await store.perform {
                try $0.snippetCreate(draft: draft("Other", "AKIAFAKEEXAMPLE00000", trigger: ";key"))
            }
            XCTFail("a duplicate trigger must not be accepted")
        } catch let error as CoreError {
            XCTAssertFalse("\(error)".contains("AKIAFAKE"))
            XCTAssertFalse(error.localizedDescription.contains("AKIAFAKE"))
        }
    }
}

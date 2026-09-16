// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// The vault's red lines, checked from the side that will render them.
///
/// The cryptography is tested in Rust. What is under test here is that the
/// platform layer cannot get at a secret it should not have: not through a
/// list, not through a search, not through an error message.
final class VaultTests: XCTestCase {
    /// Deliberately fake secret body, so every leak assertion can grep for it.
    private static let secret = "AKIAFAKEEXAMPLE00000"
    private static let password = "correct-horse-t087"

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

    private func secretDraft(_ title: String) -> SnippetDraft {
        SnippetDraft(
            title: title,
            body: Self.secret,
            snippetType: "sensitive",
            description: nil,
            folderId: nil,
            trigger: nil,
            triggerMode: nil,
            language: nil
        )
    }

    private func openVaultWithOneSecret() async throws -> Snippet {
        _ = try await store.perform { try $0.vaultInitialize(password: Self.password) }
        return try await store.perform {
            try $0.vaultCreateSecret(draft: secretDraft("Deploy key"))
        }
    }

    func testUnlockingAndLockingReportTheSessionState() async throws {
        let opened = try await store.perform { try $0.vaultInitialize(password: Self.password) }
        XCTAssertTrue(opened.initialized)
        XCTAssertTrue(opened.unlocked)

        let closed = try await store.perform { try $0.vaultLock() }
        XCTAssertTrue(closed.initialized)
        XCTAssertFalse(closed.unlocked)
        XCTAssertNil(closed.unlockedAt)

        let reopened = try await store.perform {
            try $0.vaultUnlockPassword(password: Self.password)
        }
        XCTAssertTrue(reopened.unlocked)
    }

    /// Red line: the wrong master password does not open the vault, and the
    /// refusal does not say which part was wrong.
    func testAWrongPasswordDoesNotOpenTheVault() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: Self.password) }
        try await store.perform { _ = try $0.vaultLock() }

        do {
            _ = try await store.perform {
                try $0.vaultUnlockPassword(password: "correct-horse-t088")
            }
            XCTFail("a wrong password must not open the vault")
        } catch let error as CoreError {
            guard case .PermissionDenied = error else {
                return XCTFail("expected permission denied, got \(error)")
            }
        }

        let status = try await store.perform { try $0.vaultStatus() }
        XCTAssertFalse(status.unlocked)
    }

    /// Red line: a locked vault previews nothing. The list still renders — the
    /// room keeps its shape — but every body is absent and the refusal to
    /// reveal carries no trace of the secret.
    func testALockedVaultPreviewsNothing() async throws {
        let secret = try await openVaultWithOneSecret()
        try await store.perform { _ = try $0.vaultLock() }

        let rows = try await store.perform { try $0.vaultList(limit: 20, offset: 0) }
        XCTAssertEqual(rows.count, 1)
        XCTAssertNil(rows[0].body)
        XCTAssertFalse("\(rows)".contains("AKIAFAKE"))

        do {
            _ = try await store.perform { try $0.vaultReveal(id: secret.id) }
            XCTFail("a locked vault must not reveal")
        } catch let error as CoreError {
            XCTAssertFalse("\(error)".contains("AKIAFAKE"))
        }
    }

    /// Red line: a secret's body is not searchable from any surface, and its
    /// title hit comes back as a locked row rather than as content.
    func testASecretBodyIsNotSearchableFromAnySurface() async throws {
        _ = try await openVaultWithOneSecret()

        for query in [Self.secret, "AKIAFAKE", "AKIA"] {
            let library = try await store.perform { try $0.searchLibrary(query: query, limit: 20) }
            let all = try await store.perform { try $0.searchAll(query: query, limit: 20) }
            XCTAssertTrue(library.isEmpty, "library search surfaced \(query)")
            XCTAssertTrue(all.isEmpty, "search surfaced \(query)")
        }

        let byTitle = try await store.perform { try $0.searchAll(query: "Deploy key", limit: 20) }
        XCTAssertEqual(byTitle.count, 1)
        XCTAssertNil(byTitle[0].body)
    }

    /// An open vault reveals exactly what was asked for, and only on request.
    func testAnOpenVaultRevealsOnlyTheSecretThatWasAskedFor() async throws {
        let secret = try await openVaultWithOneSecret()

        let revealed = try await store.perform { try $0.vaultReveal(id: secret.id) }

        XCTAssertEqual(revealed, Self.secret)
        let rows = try await store.perform { try $0.vaultList(limit: 20, offset: 0) }
        XCTAssertNil(rows[0].body, "revealing one secret must not open the list")
    }

    /// The confirm copy has to be able to state the real number, and it has to
    /// be able to state it while locked — losing the password is the situation
    /// a reset exists for.
    func testResetPreviewStatesTheRealCountWhileLocked() async throws {
        _ = try await openVaultWithOneSecret()
        try await store.perform { _ = try $0.vaultLock() }

        let count = try await store.perform { try $0.vaultResetPreview() }

        XCTAssertEqual(count, 1)
    }
}

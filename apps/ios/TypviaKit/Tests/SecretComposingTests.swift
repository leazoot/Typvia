// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// Fixtures at file scope so the calls that cross into the core's actor can
/// read them: deliberately fake, and shaped so a byte scan can find the body
/// if it ever leaks.
private enum Fixture {
    static let secretBody = "AKIA_FAKE_NOT_A_SECRET_deploy_value"
    static let password = "correct horse battery"
}

/// Writing a secret in the editor, against a real database.
///
/// The path this covers did not exist on this platform: the bridge could make
/// a vault and encrypt a body, and nothing in the app ever called either, so
/// no secret could be written on the device at all. What is under test is that
/// the editor now reaches the vault's door — and that the ordinary door stays
/// shut to it, so the guarantee does not rest on the screen remembering.
@MainActor
final class SecretComposingTests: XCTestCase {
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

    private func composer() -> DetailModel {
        let model = DetailModel(store: store)
        model.draft = Draft(title: "Deploy key", body: Fixture.secretBody, sort: .secret)
        return model
    }

    // MARK: - A secret is named by hand

    /// Everything else may go unnamed — the shared layer files it under its
    /// own first line. A secret may not, and the reason is the whole point: a
    /// title is searchable, so borrowing the first line of a secret would put
    /// the secret in the index. The vault refuses one either way; what is
    /// under test is that the editor asks instead of sending it.
    func testASecretWillNotBeSentWithoutAName() async throws {
        let model = DetailModel(store: store)
        model.draft = Draft(title: "  ", body: Fixture.secretBody, sort: .secret)

        XCTAssertTrue(model.draft.needsAName)
        XCTAssertFalse(model.draft.canSave)

        let written = await model.save()
        XCTAssertFalse(written)
        XCTAssertNil(model.secretGate, "nothing was sent, so nothing asked for the vault")

        model.draft.title = "Deploy key"
        XCTAssertFalse(model.draft.needsAName)
        XCTAssertTrue(model.draft.canSave)
    }

    func testEverythingElseMayStillGoUnnamed() {
        var ordinary = Draft(title: "", body: "kubectl get pods", sort: .command)
        XCTAssertFalse(ordinary.needsAName)
        XCTAssertTrue(ordinary.canSave)
        ordinary.sort = nil
        XCTAssertTrue(ordinary.canSave, "an unchosen kind is text, and text may go unnamed")
    }

    // MARK: - Making the vault the first secret needs

    func testTheFirstSecretAsksForAVaultInsteadOfRefusing() async throws {
        let model = composer()

        let written = await model.save()

        XCTAssertFalse(written)
        XCTAssertEqual(model.secretGate, .setup)
        XCTAssertNil(model.refusal, "a vault that has to be made is not a failure to report")
        XCTAssertNil(model.snippet)
    }

    func testTypingItTwiceMakesTheVaultAndPutsTheSecretStraightIn() async throws {
        let model = composer()
        _ = await model.save()

        let written = await model.makeVault(password: Fixture.password, confirmation: Fixture.password)

        XCTAssertTrue(written)
        XCTAssertNil(model.secretGate)
        XCTAssertEqual(model.snippet?.securityLevel, "sensitive")
        XCTAssertNil(model.snippet?.body, "a secret's row hands back no body")
        let listed = try await store.perform { try $0.vaultList(limit: 10, offset: 0) }
        XCTAssertEqual(listed.map(\.title), ["Deploy key"])
    }

    /// The typed plaintext leaves the screen the moment it is encrypted: the
    /// draft is rebuilt from the row that came back, and that row has no body.
    func testTheEditorStopsHoldingWhatItJustEncrypted() async throws {
        let model = composer()
        _ = await model.save()
        _ = await model.makeVault(password: Fixture.password, confirmation: Fixture.password)

        XCTAssertEqual(model.draft.body, "")
        XCTAssertNil(model.revealed)
    }

    func testTwoDifferentTypingsMakeNoVaultAndSendNothingAnywhere() async throws {
        let model = composer()
        _ = await model.save()

        let written = await model.makeVault(password: Fixture.password, confirmation: "correct horse")

        XCTAssertFalse(written)
        XCTAssertTrue(model.mismatch)
        XCTAssertNil(model.refusal, "nothing was sent, so nothing refused it")
        let status = try await store.perform { try $0.vaultStatus() }
        XCTAssertFalse(status.initialized)
    }

    /// The floor on a master password belongs to the vault. The screen submits
    /// and reports; it does not decide.
    func testAShortMasterPasswordIsTheVaultsRefusalNotTheScreensGuess() async throws {
        let model = composer()
        _ = await model.save()

        let written = await model.makeVault(password: "short", confirmation: "short")

        XCTAssertFalse(written)
        XCTAssertEqual(model.refusal, .invalid)
        XCTAssertFalse(model.mismatch)
        let status = try await store.perform { try $0.vaultStatus() }
        XCTAssertFalse(status.initialized)
    }

    func testTheMinimumTheScreenCanStateIsTheOneTheVaultEnforces() async throws {
        let minimum = Int(masterPasswordMinLength())
        XCTAssertGreaterThan(minimum, 0)

        let model = composer()
        _ = await model.save()
        let justUnder = String(repeating: "x", count: minimum - 1)
        let refused = await model.makeVault(password: justUnder, confirmation: justUnder)
        XCTAssertFalse(refused)

        let atTheFloor = String(repeating: "x", count: minimum)
        let accepted = await model.makeVault(password: atTheFloor, confirmation: atTheFloor)
        XCTAssertTrue(accepted)
    }

    // MARK: - A vault that exists but is shut

    func testAShutVaultIsOpenedRatherThanFailingTheSave() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: Fixture.password) }
        _ = try await store.perform { try $0.vaultLock() }
        let model = composer()

        let written = await model.save()

        XCTAssertFalse(written)
        XCTAssertEqual(model.secretGate, .shut)

        let opened = await model.openVault(password: Fixture.password)
        XCTAssertTrue(opened)
        XCTAssertNil(model.secretGate)
        XCTAssertEqual(model.snippet?.securityLevel, "sensitive")
    }

    func testAWrongMasterPasswordWritesNothingAndLeavesTheGateOpen() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: Fixture.password) }
        _ = try await store.perform { try $0.vaultLock() }
        let model = composer()
        _ = await model.save()

        let opened = await model.openVault(password: "not the master password")

        XCTAssertFalse(opened)
        XCTAssertEqual(model.secretGate, .shut, "the way in stays on screen")
        XCTAssertEqual(model.refusal, .notPermitted)
        XCTAssertNil(model.snippet)
    }

    func testLeavingTheGateKeepsWhatWasTyped() async throws {
        let model = composer()
        _ = await model.save()

        model.dismissGate()

        XCTAssertNil(model.secretGate)
        XCTAssertEqual(model.draft.body, Fixture.secretBody, "changing your mind about the vault is not changing your mind about the words")
    }

    // MARK: - Red lines

    /// Byte level, not field level. A secret written through the editor must
    /// not be findable in the library's index, and its body must not be
    /// anywhere in the files the database is made of — including the
    /// write-ahead log, which is where a plaintext insert would still show up
    /// after the row itself was replaced.
    func testWhatWasTypedAsASecretIsNowhereInTheLibraryOrItsFiles() async throws {
        let model = composer()
        _ = await model.save()
        let written = await model.makeVault(password: Fixture.password, confirmation: Fixture.password)
        XCTAssertTrue(written)

        let hits = try await store.perform {
            try $0.searchSnippets(query: Fixture.secretBody, limit: 20, offset: 0)
        }
        XCTAssertTrue(hits.isEmpty)
        let onTheHomeSearch = try await store.perform {
            try $0.searchAll(query: Fixture.secretBody, limit: 20)
        }
        XCTAssertTrue(onTheHomeSearch.isEmpty)

        let needle = Data(Fixture.secretBody.utf8)
        for file in try FileManager.default.contentsOfDirectory(atPath: directory.path) {
            let bytes = try Data(contentsOf: directory.appendingPathComponent(file))
            XCTAssertNil(bytes.range(of: needle), "\(file) holds the plaintext")
        }
    }

    /// The screen routes secrets to the vault, and the core refuses them at
    /// the ordinary door regardless — otherwise the guarantee would be one
    /// forgetful edit of a Swift file away from gone.
    func testTheOrdinaryDoorRefusesToFileSomethingAsASecret() async throws {
        do {
            _ = try await store.perform {
                try $0.snippetCreate(
                    draft: SnippetDraft(
                        title: "Deploy key", body: Fixture.secretBody, snippetType: "sensitive",
                        description: nil, folderId: nil, trigger: nil, triggerMode: nil,
                        language: nil
                    )
                )
            }
            XCTFail("an ordinary save must not accept the secret kind")
        } catch let error as CoreError {
            XCTAssertEqual(Refusal(error), .invalid)
        }
    }

    // MARK: - Moving something already saved into the vault

    /// Promotion is the core's own act — re-encrypt, rebuild the index, drop
    /// the plaintext history — and the screen only asks for it.
    func testASavedSnippetCanBeMovedIntoTheVault() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: Fixture.password) }
        let saved = try await store.perform {
            try $0.snippetCreate(
                draft: SnippetDraft(
                    title: "Cluster login", body: Fixture.secretBody, snippetType: "text",
                    description: nil, folderId: nil, trigger: nil, triggerMode: nil, language: nil
                )
            )
        }
        let model = DetailModel(store: store, snippetId: saved.id)
        await model.load()

        let moved = await model.promoteToSecret()

        XCTAssertTrue(moved)
        XCTAssertEqual(model.snippet?.securityLevel, "sensitive")
        XCTAssertNil(model.snippet?.body)
        // The cleartext past goes with it: what it used to be must not stay
        // searchable after it has become a secret.
        let hits = try await store.perform {
            try $0.searchSnippets(query: Fixture.secretBody, limit: 20, offset: 0)
        }
        XCTAssertTrue(hits.isEmpty)
    }

    func testMovingItAsksForTheVaultTheSameWayWritingOneDoes() async throws {
        let saved = try await store.perform {
            try $0.snippetCreate(
                draft: SnippetDraft(
                    title: "Cluster login", body: "kubectl config", snippetType: "text",
                    description: nil, folderId: nil, trigger: nil, triggerMode: nil, language: nil
                )
            )
        }
        let model = DetailModel(store: store, snippetId: saved.id)
        await model.load()

        let moved = await model.promoteToSecret()

        XCTAssertFalse(moved)
        XCTAssertEqual(model.secretGate, .setup, "no vault on this device yet")

        // And the gate finishes the act that opened it, not the other one.
        let made = await model.makeVault(password: Fixture.password, confirmation: Fixture.password)
        XCTAssertTrue(made)
        XCTAssertEqual(model.snippet?.securityLevel, "sensitive")
    }

    func testASecretIsNotOfferedTheMoveAgain() async throws {
        _ = try await store.perform { try $0.vaultInitialize(password: Fixture.password) }
        let secret = try await store.perform {
            try $0.vaultCreateSecret(
                draft: SnippetDraft(
                    title: "Deploy key", body: Fixture.secretBody, snippetType: "sensitive",
                    description: nil, folderId: nil, trigger: nil, triggerMode: nil, language: nil
                )
            )
        }
        let model = DetailModel(store: store, snippetId: secret.id)
        await model.load()

        let moved = await model.promoteToSecret()

        XCTAssertFalse(moved, "it is already where it belongs")
    }

    func testAnExistingSnippetIsNotOfferedTheSecretKind() {
        XCTAssertEqual(Draft.offeredKinds(isNew: true), Chapter.order)
        XCTAssertFalse(Draft.offeredKinds(isNew: false).contains(.secret))
        XCTAssertEqual(Draft.offeredKinds(isNew: false).count, Chapter.order.count - 1)
    }
}

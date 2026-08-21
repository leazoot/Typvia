// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// SPIKE: iOS Keychain round trip + biometric availability probe,
// executed inside the simulator via a hosted unit test.

import LocalAuthentication
import Security
import XCTest

final class SecureStorageTests: XCTestCase {
    let service = "dev.typvia.spike.secure-storage"
    let account = "master-key"
    let fakeKey = "SPIKE_FAKE_KEY_MATERIAL_0123456789".data(using: .utf8)!

    func deleteItem() {
        let q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(q as CFDictionary)
    }

    func testKeychainRoundTrip() {
        deleteItem()
        let add: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: fakeKey,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
        ]
        XCTAssertEqual(SecItemAdd(add as CFDictionary, nil), errSecSuccess, "store")

        let get: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
        ]
        var out: CFTypeRef?
        XCTAssertEqual(SecItemCopyMatching(get as CFDictionary, &out), errSecSuccess, "retrieve")
        XCTAssertEqual(out as? Data, fakeKey, "bytes match")
        deleteItem()
        print("SPIKE-secure: ios keychain roundtrip OK")
    }

    func testBiometricAvailability() {
        let ctx = LAContext()
        var err: NSError?
        let can = ctx.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &err)
        // Simulator without enrolled biometrics reports false — record, not assert.
        print("SPIKE-secure: ios biometrics can=\(can) type=\(ctx.biometryType.rawValue) err=\(err?.code ?? 0)")

        var acErr: Unmanaged<CFError>?
        let ac = SecAccessControlCreateWithFlags(
            nil, kSecAttrAccessibleWhenUnlockedThisDeviceOnly, .userPresence, &acErr)
        XCTAssertNotNil(ac, "userPresence access control constructible")
        print("SPIKE-secure: ios access control constructible=\(ac != nil)")
    }
}

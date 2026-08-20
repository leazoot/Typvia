// SPIKE: macOS Keychain store/retrieve round trip plus biometric
// availability probe. Key material is an obvious fake — never real secrets.

import Foundation
import LocalAuthentication
import Security

let service = "dev.typvia.spike.secure-storage"
let account = "master-key"
let fakeKey = "SPIKE_FAKE_KEY_MATERIAL_0123456789".data(using: .utf8)!

func delete() {
    let q: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: service,
        kSecAttrAccount as String: account,
    ]
    SecItemDelete(q as CFDictionary)
}

// Store.
delete()
let add: [String: Any] = [
    kSecClass as String: kSecClassGenericPassword,
    kSecAttrService as String: service,
    kSecAttrAccount as String: account,
    kSecValueData as String: fakeKey,
]
let addStatus = SecItemAdd(add as CFDictionary, nil)
print("keychain_store: \(addStatus == errSecSuccess ? "OK" : "FAILED \(addStatus)")")

// Retrieve.
let get: [String: Any] = [
    kSecClass as String: kSecClassGenericPassword,
    kSecAttrService as String: service,
    kSecAttrAccount as String: account,
    kSecReturnData as String: true,
]
var out: CFTypeRef?
let getStatus = SecItemCopyMatching(get as CFDictionary, &out)
let roundtrip = (out as? Data) == fakeKey
print("keychain_retrieve: \(getStatus == errSecSuccess && roundtrip ? "OK (bytes match)" : "FAILED \(getStatus)")")
delete()
print("keychain_cleanup: done")

// Biometric availability (no interactive prompt — capability probe only).
let ctx = LAContext()
var err: NSError?
let canBio = ctx.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &err)
let type = switch ctx.biometryType {
case .touchID: "touchID"
case .faceID: "faceID"
case .opticID: "opticID"
default: "none"
}
print("biometrics_available: \(canBio) type=\(type) err=\(err?.code.description ?? "-")")

// Access-control capability: building a userPresence-protected item spec must
// succeed; the interactive prompt itself is not automatable headlessly.
var acErr: Unmanaged<CFError>?
let ac = SecAccessControlCreateWithFlags(
    nil, kSecAttrAccessibleWhenUnlockedThisDeviceOnly, .userPresence, &acErr)
print("access_control_userPresence: \(ac != nil ? "constructible" : "FAILED")")

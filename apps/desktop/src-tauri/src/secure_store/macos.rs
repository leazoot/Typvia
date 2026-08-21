// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! macOS Keychain `SecureStore` backed by Security.framework. This mirrors the
//! Swift protocol validated in the secure-storage spike:
//! `SecAccessControl(.userPresence)` + `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`.
//!
//! Two entry classes share the store, and the difference is a red line rather
//! than a preference:
//!
//! - the vault master-key copy is gated on `userPresence` (Touch ID /
//!   passcode) at read time;
//! - the sync device identity and the K_sync generations are **not** gated,
//!   because offline sync of ordinary snippets has to work while the vault is
//!   locked. Gating them would put a biometric prompt in front of every
//!   background sync round.
//!
//! The gate is what puts an item in the data-protection keychain, which needs
//! a signed application with a keychain access group; ungated items use the
//! file-based keychain and work in an unsigned development build too.
//!
//! The interactive biometric prompt on `retrieve` cannot be exercised
//! headlessly, so the real store→prompt→retrieve round-trip is covered by the
//! opt-in live test (tests/keychain_live.rs).

use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::data::{CFData, CFDataRef};
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::{CFString, CFStringRef};
use zeroize::Zeroizing;

use typvia_crypto::{SecureStore, SecureStoreError};

type OSStatus = i32;
type CFOptionFlags = usize;
type SecAccessControlRef = *const std::ffi::c_void;
type CFErrorRef = *const std::ffi::c_void;
type CFAllocatorRef = *const std::ffi::c_void;

// Security.framework OSStatus codes we branch on.
const ERR_SEC_SUCCESS: OSStatus = 0;
const ERR_SEC_ITEM_NOT_FOUND: OSStatus = -25300;
const ERR_SEC_USER_CANCELED: OSStatus = -128;
const ERR_SEC_AUTH_FAILED: OSStatus = -25293;
const ERR_SEC_INTERACTION_NOT_ALLOWED: OSStatus = -25308;

/// `kSecAccessControlUserPresence` (bit 0): biometry with passcode fallback.
const USER_PRESENCE: CFOptionFlags = 1;

/// Keychain service namespace for all Typvia vault entries.
const SERVICE: &str = "com.typvia.vault";

/// Dedicated entry for the opt-in live test (tests/keychain_live.rs): gated
/// exactly like the vault MK copy, so the test exercises the real
/// access-control path without ever touching the real MK entry.
pub const LIVE_TEST_GATED_ENTRY: &str = "vault.test.keychain_live";

/// Whether an entry must ask for the user's presence when it is read.
/// The vault master-key copy does (plus the live test's stand-in for it);
/// everything else is sync key material that has to stay usable unattended.
fn needs_user_presence(entry: &str) -> bool {
    entry == typvia_core::vault::BIOMETRIC_MK_ENTRY || entry == LIVE_TEST_GATED_ENTRY
}

#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    static kSecClass: CFStringRef;
    static kSecClassGenericPassword: CFStringRef;
    static kSecAttrService: CFStringRef;
    static kSecAttrAccount: CFStringRef;
    static kSecValueData: CFStringRef;
    static kSecReturnData: CFStringRef;
    static kSecMatchLimit: CFStringRef;
    static kSecMatchLimitOne: CFStringRef;
    static kSecAttrAccessControl: CFStringRef;
    static kSecAttrAccessible: CFStringRef;
    static kSecUseOperationPrompt: CFStringRef;
    static kSecAttrAccessibleWhenUnlockedThisDeviceOnly: CFStringRef;

    fn SecItemAdd(attributes: CFTypeRef, result: *mut CFTypeRef) -> OSStatus;
    fn SecItemCopyMatching(query: CFTypeRef, result: *mut CFTypeRef) -> OSStatus;
    fn SecItemDelete(query: CFTypeRef) -> OSStatus;
    fn SecAccessControlCreateWithFlags(
        allocator: CFAllocatorRef,
        protection: CFTypeRef,
        flags: CFOptionFlags,
        error: *mut CFErrorRef,
    ) -> SecAccessControlRef;
}

/// The Security.framework CFString constants this backend uses, wrapped once.
struct Attr {
    class: CFType,
    generic_password: CFType,
    service: CFType,
    account: CFType,
    value_data: CFType,
    return_data: CFType,
    match_limit: CFType,
    match_limit_one: CFType,
    access_control: CFType,
    prompt: CFType,
    accessible_key: CFType,
    accessible: CFType,
}

impl Attr {
    fn load() -> Self {
        // Safety: each is a Security.framework CFStringRef constant, valid for
        // the process lifetime; `wrap_under_get_rule` retains a reference.
        unsafe {
            Self {
                class: wrap(kSecClass),
                generic_password: wrap(kSecClassGenericPassword),
                service: wrap(kSecAttrService),
                account: wrap(kSecAttrAccount),
                value_data: wrap(kSecValueData),
                return_data: wrap(kSecReturnData),
                match_limit: wrap(kSecMatchLimit),
                match_limit_one: wrap(kSecMatchLimitOne),
                access_control: wrap(kSecAttrAccessControl),
                prompt: wrap(kSecUseOperationPrompt),
                accessible_key: wrap(kSecAttrAccessible),
                accessible: wrap(kSecAttrAccessibleWhenUnlockedThisDeviceOnly),
            }
        }
    }
}

/// Wraps a Security.framework CFString constant as an owned [`CFType`].
///
/// # Safety
/// `constant` must be a valid, process-lifetime `CFStringRef` constant.
unsafe fn wrap(constant: CFStringRef) -> CFType {
    unsafe { CFString::wrap_under_get_rule(constant) }.as_CFType()
}

/// macOS Keychain backend. Stateless: each call builds its own query.
#[derive(Default)]
pub struct MacosKeychain;

impl MacosKeychain {
    pub fn new() -> Self {
        Self
    }

    /// The `{class, service, account}` triple that identifies one entry.
    fn identity(attr: &Attr, entry: &str) -> [(CFType, CFType); 3] {
        [
            (attr.class.clone(), attr.generic_password.clone()),
            (attr.service.clone(), CFString::new(SERVICE).as_CFType()),
            (attr.account.clone(), CFString::new(entry).as_CFType()),
        ]
    }

    fn delete_entry(attr: &Attr, entry: &str) -> Result<(), SecureStoreError> {
        let query = CFDictionary::from_CFType_pairs(&Self::identity(attr, entry));
        // Safety: `query` is a valid CFDictionary for the duration of the call.
        let status = unsafe { SecItemDelete(query.as_CFTypeRef()) };
        match status {
            ERR_SEC_SUCCESS | ERR_SEC_ITEM_NOT_FOUND => Ok(()),
            other => Err(map_error(other)),
        }
    }
}

impl SecureStore for MacosKeychain {
    fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError> {
        let attr = Attr::load();
        // Replace any prior copy so re-enabling biometrics is deterministic.
        Self::delete_entry(&attr, entry)?;

        let data = CFData::from_buffer(secret);

        let mut pairs = Vec::from(Self::identity(&attr, entry));
        pairs.push((attr.value_data.clone(), data.as_CFType()));
        if needs_user_presence(entry) {
            pairs.push((attr.access_control.clone(), create_access_control(&attr)?));
        } else {
            // Same protection class, no gate: available whenever this Mac is
            // unlocked, never synced to another device or to iCloud.
            pairs.push((attr.accessible_key.clone(), attr.accessible.clone()));
        }
        let attributes = CFDictionary::from_CFType_pairs(&pairs);

        // Safety: `attributes` is a valid CFDictionary; no result is requested.
        let status = unsafe { SecItemAdd(attributes.as_CFTypeRef(), std::ptr::null_mut()) };
        if status == ERR_SEC_SUCCESS {
            Ok(())
        } else {
            Err(map_error(status))
        }
    }

    fn retrieve(&self, entry: &str) -> Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError> {
        let attr = Attr::load();

        let mut pairs = Vec::from(Self::identity(&attr, entry));
        pairs.push((
            attr.return_data.clone(),
            CFBoolean::true_value().as_CFType(),
        ));
        pairs.push((attr.match_limit.clone(), attr.match_limit_one.clone()));
        if needs_user_presence(entry) {
            // The prompt string is what the user reads in the system sheet.
            let prompt = CFString::new("Unlock your Typvia vault");
            pairs.push((attr.prompt.clone(), prompt.as_CFType()));
        }
        let query = CFDictionary::from_CFType_pairs(&pairs);

        let mut result: CFTypeRef = std::ptr::null();
        // Safety: `query` is valid; on success `result` is a retained CFDataRef
        // (create rule) that we take ownership of via `wrap_under_create_rule`.
        let status = unsafe { SecItemCopyMatching(query.as_CFTypeRef(), &mut result) };
        match status {
            ERR_SEC_SUCCESS => {
                if result.is_null() {
                    return Ok(None);
                }
                let data = unsafe { CFData::wrap_under_create_rule(result as CFDataRef) };
                Ok(Some(Zeroizing::new(data.bytes().to_vec())))
            }
            ERR_SEC_ITEM_NOT_FOUND => Ok(None),
            other => Err(map_error(other)),
        }
    }

    fn remove(&self, entry: &str) -> Result<(), SecureStoreError> {
        Self::delete_entry(&Attr::load(), entry)
    }
}

/// Builds a `userPresence` access control tied to this device's unlocked state.
fn create_access_control(attr: &Attr) -> Result<CFType, SecureStoreError> {
    let mut error: CFErrorRef = std::ptr::null();
    // Safety: `accessible` is a valid protection constant; a null allocator
    // means the default. On failure the function returns null (and may set
    // `error`, which we do not read — no secret is involved).
    let access = unsafe {
        SecAccessControlCreateWithFlags(
            std::ptr::null(),
            attr.accessible.as_CFTypeRef(),
            USER_PRESENCE,
            &mut error,
        )
    };
    if access.is_null() {
        return Err(SecureStoreError::Backend(
            "access control unavailable".to_string(),
        ));
    }
    // Create rule: take ownership so the ref releases when the dictionary that
    // holds it is dropped.
    Ok(unsafe { CFType::wrap_under_create_rule(access as CFTypeRef) })
}

/// Maps an OSStatus to a store error. The message is a numeric code only —
/// never key material (log red line).
fn map_error(status: OSStatus) -> SecureStoreError {
    match status {
        ERR_SEC_USER_CANCELED | ERR_SEC_AUTH_FAILED => SecureStoreError::AccessDenied,
        ERR_SEC_INTERACTION_NOT_ALLOWED => SecureStoreError::Unavailable,
        other => SecureStoreError::Backend(format!("keychain OSStatus {other}")),
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Platform secure storage for the native mobile host.
//!
//! Two entry classes share this store, and the difference is a red line rather
//! than a preference (the desktop backend draws the same one):
//!
//! - the vault master-key copy ([`typvia_core::vault::BIOMETRIC_MK_ENTRY`]) is
//!   gated on the user's presence — the read itself presents Face ID;
//! - the sync device identity and the K_sync generations are **not** gated,
//!   because offline sync of ordinary snippets has to work while the vault is
//!   locked and while the app is returning to the foreground. Gating them
//!   would put a biometric sheet in front of every sync round, and would make
//!   sync outright impossible on a phone with no enrolled biometric.
//!
//! [`needs_user_presence`] is the single classification the backend reads, so
//! the rule cannot drift between entry kinds.
//!
//! The gate is the Keychain access control, not a prompt: because the MK copy
//! carries `SecAccessControl(.userPresence)`, the `SecItemCopyMatching` read
//! *is* what presents the Face ID / passcode sheet. A separate biometric
//! prompt would only show a sheet while the real gate stayed elsewhere, so
//! none is used. Swift's part of this is to decide whether to attempt the
//! read at all; it never sees key material either way.
//!
//! Items live in the app's data-protection keychain, are never synced to
//! iCloud and never restored to another device. The keyboard extension cannot
//! reach the gated entry: it is outside the app's keychain access scope.

/// Whether an entry must ask for the user's presence when it is read.
/// Exactly one entry does: the vault master-key copy. Everything else in the
/// store is key material that has to stay usable unattended.
///
/// Compiled for the platform backend that branches on it and for host test
/// runs, where the classification itself is under test.
#[cfg(any(target_os = "ios", test))]
pub(crate) fn needs_user_presence(entry: &str) -> bool {
    entry == typvia_core::vault::BIOMETRIC_MK_ENTRY
}

#[cfg(not(target_os = "ios"))]
use typvia_core::vault::{SecureStore, SecureStoreError};
#[cfg(not(target_os = "ios"))]
use zeroize::Zeroizing;

/// Stand-in where no platform backend exists (host test runs, and the
/// simulator-adjacent builds that are not iOS). Storing and retrieving report
/// `Unavailable`, so biometric unlock degrades to the master-password path
/// instead of failing the app; removal is a no-op so disabling biometrics
/// stays idempotent.
#[cfg(not(target_os = "ios"))]
#[derive(Default)]
pub struct UnavailableStore;

#[cfg(not(target_os = "ios"))]
impl SecureStore for UnavailableStore {
    fn store(&self, _entry: &str, _secret: &[u8]) -> Result<(), SecureStoreError> {
        Err(SecureStoreError::Unavailable)
    }

    fn retrieve(&self, _entry: &str) -> Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError> {
        Err(SecureStoreError::Unavailable)
    }

    fn remove(&self, _entry: &str) -> Result<(), SecureStoreError> {
        Ok(())
    }
}

/// A key store the host itself provides.
///
/// iOS reaches its Keychain from Rust, because Security.framework is a C API
/// this crate can call directly. Android's Keystore is not: it is Java, and
/// the only sane way in is from the host's own side. So the host implements
/// this, and everything above it — the sync device identity, the sync key
/// generations — stops caring which platform it is on.
///
/// **What an implementation must promise**: the bytes handed to `store` end
/// up somewhere only this application can read, sealed by a key the
/// application cannot export; `retrieve` returns exactly what was stored or
/// nothing at all; `remove` on a missing entry is not a failure. Nothing here
/// may log an entry's value — these are keys.
#[uniffi::export(with_foreign)]
pub trait KeyKeeper: Send + Sync {
    fn store(&self, entry: String, secret: Vec<u8>) -> Result<(), KeyKeeperError>;

    /// The stored bytes, or nothing when the entry was never written.
    fn retrieve(&self, entry: String) -> Result<Option<Vec<u8>>, KeyKeeperError>;

    fn remove(&self, entry: String) -> Result<(), KeyKeeperError>;
}

/// Why a host's key store could not do what was asked.
///
/// Three cases and no detail: a diagnostic from inside a key store is a
/// sentence about key material, and this one crosses a language boundary
/// where it would end up in a log.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum KeyKeeperError {
    /// This device has no usable key store. Not a failure to report — the
    /// product keeps working locally and sync says it is unavailable.
    Unavailable,
    /// The store exists and refused (a gate the user declined).
    Denied,
    /// Anything else. Carries nothing.
    Failed,
}

impl std::fmt::Display for KeyKeeperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => write!(f, "no key store on this device"),
            Self::Denied => write!(f, "the key store refused"),
            Self::Failed => write!(f, "the key store failed"),
        }
    }
}

impl std::error::Error for KeyKeeperError {}

/// Adapts a host-provided key store to the contract the vault and the sync
/// host are written against. It translates and nothing else: no caching, no
/// retries, no second copy of a secret anywhere on this side.
#[cfg(not(target_os = "ios"))]
pub(crate) struct HostKeyStore {
    keeper: std::sync::Arc<dyn KeyKeeper>,
}

#[cfg(not(target_os = "ios"))]
impl HostKeyStore {
    pub(crate) fn new(keeper: std::sync::Arc<dyn KeyKeeper>) -> Self {
        Self { keeper }
    }
}

#[cfg(not(target_os = "ios"))]
impl SecureStore for HostKeyStore {
    fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError> {
        self.keeper
            .store(entry.to_string(), secret.to_vec())
            .map_err(store_error)
    }

    fn retrieve(&self, entry: &str) -> Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError> {
        self.keeper
            .retrieve(entry.to_string())
            .map(|found| found.map(Zeroizing::new))
            .map_err(store_error)
    }

    fn remove(&self, entry: &str) -> Result<(), SecureStoreError> {
        self.keeper.remove(entry.to_string()).map_err(store_error)
    }
}

#[cfg(not(target_os = "ios"))]
fn store_error(error: KeyKeeperError) -> SecureStoreError {
    match error {
        KeyKeeperError::Unavailable => SecureStoreError::Unavailable,
        KeyKeeperError::Denied => SecureStoreError::AccessDenied,
        // The host's own words are deliberately not carried across: this
        // string is written into diagnostics, and a key store's diagnostics
        // are about keys.
        KeyKeeperError::Failed => SecureStoreError::Backend("host key store".into()),
    }
}

/// The secure store for the current host: the Face ID-gated Keychain on iOS,
/// the unavailable fallback elsewhere so startup never fails and the
/// master-password path stays usable.
pub(crate) fn platform_secure_store()
-> Box<dyn typvia_core::vault::SecureStore + Send + Sync + 'static> {
    #[cfg(target_os = "ios")]
    {
        Box::new(ios::IosKeychain::new())
    }
    #[cfg(not(target_os = "ios"))]
    {
        Box::new(UnavailableStore)
    }
}

#[cfg(target_os = "ios")]
mod ios {
    //! iOS Keychain backend. Every Security.framework constant and function
    //! used here exists unchanged on macOS, where the desktop host uses the
    //! same approach; the two hosts share no crate, and extracting one for
    //! ~200 lines would be more structure than the duplication costs.

    use core_foundation::base::{CFType, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::data::{CFData, CFDataRef};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::{CFString, CFStringRef};
    use zeroize::Zeroizing;

    use typvia_core::vault::{SecureStore, SecureStoreError};

    type OSStatus = i32;
    type CFOptionFlags = usize;
    type SecAccessControlRef = *const std::ffi::c_void;
    type CFErrorRef = *const std::ffi::c_void;
    type CFAllocatorRef = *const std::ffi::c_void;

    // Security.framework OSStatus codes this backend branches on.
    const ERR_SEC_SUCCESS: OSStatus = 0;
    const ERR_SEC_ITEM_NOT_FOUND: OSStatus = -25300;
    const ERR_SEC_USER_CANCELED: OSStatus = -128;
    const ERR_SEC_AUTH_FAILED: OSStatus = -25293;
    const ERR_SEC_INTERACTION_NOT_ALLOWED: OSStatus = -25308;

    /// `kSecAccessControlUserPresence` (bit 0): biometry with passcode fallback.
    const USER_PRESENCE: CFOptionFlags = 1;

    /// Keychain service namespace for every Typvia vault entry. iOS keychains
    /// are app-scoped, so this cannot collide with another app.
    const SERVICE: &str = "com.typvia.vault";

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
            // Safety: each is a Security.framework CFStringRef constant, valid
            // for the process lifetime; `wrap_under_get_rule` retains a
            // reference.
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

    /// iOS Keychain backend. Stateless: each call builds its own query.
    #[derive(Default)]
    pub struct IosKeychain;

    impl IosKeychain {
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
            // Safety: `query` is a valid CFDictionary for the call's duration.
            let status = unsafe { SecItemDelete(query.as_CFTypeRef()) };
            match status {
                ERR_SEC_SUCCESS | ERR_SEC_ITEM_NOT_FOUND => Ok(()),
                other => Err(map_error(other)),
            }
        }
    }

    impl SecureStore for IosKeychain {
        fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError> {
            let attr = Attr::load();
            // Replace any prior copy so re-enabling biometrics is deterministic.
            Self::delete_entry(&attr, entry)?;

            let data = CFData::from_buffer(secret);

            let mut pairs = Vec::from(Self::identity(&attr, entry));
            pairs.push((attr.value_data.clone(), data.as_CFType()));
            if super::needs_user_presence(entry) {
                pairs.push((attr.access_control.clone(), create_access_control(&attr)?));
            } else {
                // Same protection class, no gate: readable whenever this
                // device is unlocked (including a locked vault and a sync
                // round on return to the foreground), never restored to
                // another device and never synced to iCloud.
                pairs.push((attr.accessible_key.clone(), attr.accessible.clone()));
            }
            let attributes = CFDictionary::from_CFType_pairs(&pairs);

            // Safety: `attributes` is a valid CFDictionary; no result requested.
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
            if super::needs_user_presence(entry) {
                // The prompt string is what the user reads in the system sheet.
                let prompt = CFString::new("Unlock your Typvia vault");
                pairs.push((attr.prompt.clone(), prompt.as_CFType()));
            }
            let query = CFDictionary::from_CFType_pairs(&pairs);

            let mut result: CFTypeRef = std::ptr::null();
            // Safety: `query` is valid; on success `result` is a retained
            // CFDataRef (create rule) we take ownership of via
            // `wrap_under_create_rule`. For a gated entry this call blocks on
            // the Face ID / passcode sheet the access control demands; an
            // ungated entry returns without any user interaction.
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

    /// Builds a `userPresence` access control tied to this device's unlocked
    /// state.
    fn create_access_control(attr: &Attr) -> Result<CFType, SecureStoreError> {
        let mut error: CFErrorRef = std::ptr::null();
        // Safety: `accessible` is a valid protection constant; a null allocator
        // means the default. On failure the function returns null (and may set
        // `error`, which is not read — no secret is involved).
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
        // Create rule: take ownership so the ref releases when the dictionary
        // holding it drops.
        Ok(unsafe { CFType::wrap_under_create_rule(access as CFTypeRef) })
    }

    /// Maps an OSStatus to a store error. The message is a numeric code only,
    /// never key material.
    fn map_error(status: OSStatus) -> SecureStoreError {
        match status {
            ERR_SEC_USER_CANCELED | ERR_SEC_AUTH_FAILED => SecureStoreError::AccessDenied,
            ERR_SEC_INTERACTION_NOT_ALLOWED => SecureStoreError::Unavailable,
            other => SecureStoreError::Backend(format!("keychain OSStatus {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HostKeyStore, KeyKeeper, KeyKeeperError, SecureStore, SecureStoreError, needs_user_presence,
    };
    use typvia_core::vault::BIOMETRIC_MK_ENTRY;

    /// Red line: the vault master-key copy is gated on the user's presence.
    /// The gate is the Keychain access control on this one entry, so getting
    /// the classification wrong is the difference between a protected key and
    /// an unprotected one.
    #[test]
    fn the_vault_master_key_copy_is_gated_on_user_presence() {
        assert!(needs_user_presence(BIOMETRIC_MK_ENTRY));
    }

    /// The converse red line: nothing else is gated. An accidental match here
    /// would raise a biometric sheet on paths that must run unattended.
    #[test]
    fn an_entry_that_merely_looks_like_it_is_not_gated() {
        assert!(!needs_user_presence("vault.mk"));
        assert!(!needs_user_presence("vault.mk.biometric.old"));
        assert!(!needs_user_presence(""));
    }
    /// A host key store that answers however the test needs it to.
    struct Keeper(Option<KeyKeeperError>);

    impl KeyKeeper for Keeper {
        fn store(&self, _entry: String, _secret: Vec<u8>) -> Result<(), KeyKeeperError> {
            match &self.0 {
                Some(error) => Err(error.clone()),
                None => Ok(()),
            }
        }

        fn retrieve(&self, entry: String) -> Result<Option<Vec<u8>>, KeyKeeperError> {
            match &self.0 {
                Some(error) => Err(error.clone()),
                None if entry == "known" => Ok(Some(vec![7, 7, 7])),
                None => Ok(None),
            }
        }

        fn remove(&self, _entry: String) -> Result<(), KeyKeeperError> {
            match &self.0 {
                Some(error) => Err(error.clone()),
                None => Ok(()),
            }
        }
    }

    fn store_with(error: Option<KeyKeeperError>) -> HostKeyStore {
        HostKeyStore::new(std::sync::Arc::new(Keeper(error)))
    }

    /// "No key store on this device" and "the store refused" are different
    /// facts about the device, and the layers above branch on them: the first
    /// makes sync report itself unavailable, the second is a user who said no.
    /// Collapsing them would make a phone with a working key store look like
    /// one without.
    #[test]
    fn each_kind_of_host_refusal_keeps_its_own_meaning() {
        assert!(matches!(
            store_with(Some(KeyKeeperError::Unavailable)).store("e", b"k"),
            Err(SecureStoreError::Unavailable)
        ));
        assert!(matches!(
            store_with(Some(KeyKeeperError::Denied)).store("e", b"k"),
            Err(SecureStoreError::AccessDenied)
        ));
        assert!(matches!(
            store_with(Some(KeyKeeperError::Failed)).store("e", b"k"),
            Err(SecureStoreError::Backend(_))
        ));
    }

    /// A missing entry is `None`, never an error: the first launch on a device
    /// reads every entry before it writes any of them, and a failure there
    /// would stop the device from ever getting an identity.
    #[test]
    fn a_missing_entry_reads_as_nothing_rather_than_as_a_failure() {
        let store = store_with(None);

        assert!(store.retrieve("unwritten").unwrap().is_none());
        assert_eq!(
            store.retrieve("known").unwrap().unwrap().as_slice(),
            &[7, 7, 7]
        );
    }

    /// Removing what is not there is not a failure — a host that reported one
    /// would make disabling something twice an error the second time.
    #[test]
    fn removing_is_idempotent_through_the_adapter() {
        assert!(store_with(None).remove("gone").is_ok());
    }
}

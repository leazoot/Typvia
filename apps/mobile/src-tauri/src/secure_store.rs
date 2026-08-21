// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Platform secure storage for the mobile host.
//!
//! Two entry classes share this store, and the difference is a red line rather
//! than a preference (mirrors the desktop backend):
//!
//! - the vault master-key copy ([`typvia_core::vault::BIOMETRIC_MK_ENTRY`]) is
//!   gated on the user's presence — the read itself presents Face ID on iOS
//!   and the BiometricPrompt-bound Keystore decrypt on Android;
//! - the sync device identity and the K_sync generations are **not** gated,
//!   because offline sync of ordinary snippets has to work while the vault is
//!   locked and while the app is coming back to the foreground. Gating them
//!   would put a biometric sheet in front of every sync round, and would make
//!   sync outright impossible on a phone with no enrolled biometric.
//!
//! [`needs_user_presence`] is the single classification both platform backends
//! read, so the two hosts can never drift apart on which entry is gated.
//!
//! The iOS backend is copy-adapted from the desktop macOS backend
//! (apps/desktop/src-tauri/src/secure_store/macos.rs): Security.framework and
//! every constant/function used there exist unchanged on iOS, and the
//! `SecAccessControl(.userPresence)` gate means the `SecItemCopyMatching` read
//! itself presents the Face ID / passcode sheet. That makes a separate prompt
//! plugin (tauri-plugin-biometric) unnecessary: it would only show a prompt,
//! while the actual gate on the MK copy is this Keychain access control — so
//! the plugin is deliberately not added.
//!
//! iOS differences from macOS are behavioural, not API-level: items live in
//! the app's data-protection keychain (no login keychain), the gate is Face ID
//! (needs `NSFaceIDUsageDescription`), and the keyboard extension cannot reach
//! this entry because it is not in the app's keychain access scope.
//!
//! The Android backend mirrors the same "the read is the gate"
//! decision: a Keystore-resident RSA key whose private half demands a per-use
//! STRONG biometric wraps the MK copy, and the system BiometricPrompt is bound
//! to that very decrypt via `CryptoObject`. `tauri-plugin-biometric` was
//! evaluated again and again not introduced — it only shows a prompt, it is
//! not the gate (same reasoning as the iOS decision above); androidx.biometric
//! was not introduced either because the framework
//! `android.hardware.biometrics.BiometricPrompt` (API 28+) matches minSdk 28
//! exactly, so the library would add a dependency purely for backports below
//! our floor. Ungated entries take the Keystore AES-GCM path in the same
//! Kotlin object. See the `android` module and TypviaSecureStore.kt for the
//! scheme details and the Rust↔Kotlin handshake.

/// Whether an entry must ask for the user's presence when it is read.
/// Exactly one entry does: the vault master-key copy. Everything else in the
/// store is sync key material that has to stay usable unattended (see the
/// module comment).
///
/// Compiled for the two platform hosts that branch on it and for host test
/// runs, where the classification is pinned by the tests below.
#[cfg(any(target_os = "ios", target_os = "android", test))]
pub(crate) fn needs_user_presence(entry: &str) -> bool {
    entry == typvia_core::vault::BIOMETRIC_MK_ENTRY
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
use typvia_core::vault::{SecureStore, SecureStoreError};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use zeroize::Zeroizing;

/// Stand-in when no platform backend exists (the desktop dev shell and unit
/// tests). Storing/retrieving report `Unavailable` so biometrics degrade to
/// the master-password path; removal is a no-op so disabling biometrics stays
/// idempotent.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub struct UnavailableStore;

#[cfg(not(any(target_os = "ios", target_os = "android")))]
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

/// The secure store for the current host: the Face ID-gated Keychain on iOS,
/// the BiometricPrompt-gated Keystore wrap on Android, the unavailable
/// fallback elsewhere so startup never fails and the master-password path
/// stays usable. The handle is only needed by the Android backend (one-time
/// JVM capture); the other hosts ignore it.
pub fn platform_secure_store_or_unavailable(
    #[allow(unused_variables)] app: &tauri::AppHandle,
) -> Box<dyn typvia_core::vault::SecureStore + Send + Sync> {
    #[cfg(target_os = "ios")]
    {
        Box::new(ios::IosKeychain::new())
    }
    #[cfg(target_os = "android")]
    {
        Box::new(android::AndroidKeystore::new(app.clone()))
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        Box::new(UnavailableStore)
    }
}

#[cfg(target_os = "ios")]
mod ios {
    //! iOS Keychain backend. Copy-adapted from the desktop macOS backend;
    //! the desktop file stays untouched because the two hosts do not share a
    //! crate and extracting one would create a new crate for ~200
    //! lines (rejected as over-structure — recorded here instead).

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

    // Security.framework OSStatus codes we branch on.
    const ERR_SEC_SUCCESS: OSStatus = 0;
    const ERR_SEC_ITEM_NOT_FOUND: OSStatus = -25300;
    const ERR_SEC_USER_CANCELED: OSStatus = -128;
    const ERR_SEC_AUTH_FAILED: OSStatus = -25293;
    const ERR_SEC_INTERACTION_NOT_ALLOWED: OSStatus = -25308;

    /// `kSecAccessControlUserPresence` (bit 0): biometry with passcode fallback.
    const USER_PRESENCE: CFOptionFlags = 1;

    /// Keychain service namespace for all Typvia vault entries (same name as
    /// desktop; iOS keychains are app-scoped, so there is no collision).
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
            // for the process lifetime; `wrap_under_get_rule` retains a reference.
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
            // Safety: `query` is a valid CFDictionary for the duration of the call.
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
        // Create rule: take ownership so the ref releases when the dictionary
        // that holds it is dropped.
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
}

/// Status protocol and completion routing shared with TypviaSecureStore.kt.
/// Platform-free by design and therefore also compiled for host test runs,
/// where the routing/mapping logic is unit-tested; only the `android` module
/// below performs platform I/O.
#[cfg(any(target_os = "android", test))]
pub(crate) mod android_protocol {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
    use std::sync::{Mutex, OnceLock};

    use typvia_core::vault::SecureStoreError;
    use zeroize::Zeroizing;

    // Status codes; TypviaSecureStore.kt holds the same table and the two
    // must stay in lockstep (the values cross JNI as plain ints).
    pub const STATUS_OK: i32 = 0;
    pub const STATUS_PENDING: i32 = 1;
    pub const STATUS_NOT_FOUND: i32 = 2;
    pub const STATUS_UNAVAILABLE: i32 = 3;
    pub const STATUS_DENIED: i32 = 4;

    pub type RetrieveOutcome = Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError>;

    fn waiters() -> &'static Mutex<HashMap<i64, SyncSender<RetrieveOutcome>>> {
        static WAITERS: OnceLock<Mutex<HashMap<i64, SyncSender<RetrieveOutcome>>>> =
            OnceLock::new();
        WAITERS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// The waiter map never holds its lock across a panic; recovering from a
    /// theoretical poison keeps later unlock attempts working.
    fn lock_waiters() -> std::sync::MutexGuard<'static, HashMap<i64, SyncSender<RetrieveOutcome>>> {
        waiters()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Registers a waiter for one gated read; the id travels to Kotlin and
    /// back so the async completion finds its way home.
    pub fn register_waiter() -> (i64, Receiver<RetrieveOutcome>) {
        static NEXT_ID: AtomicI64 = AtomicI64::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        // Capacity 1 so a completion racing the timeout never blocks the
        // main executor delivering it.
        let (sender, receiver) = sync_channel(1);
        lock_waiters().insert(id, sender);
        (id, receiver)
    }

    /// Drops a waiter that timed out or failed before the prompt resolved;
    /// a completion arriving later is then ignored by [`complete_waiter`].
    pub fn deregister_waiter(id: i64) {
        lock_waiters().remove(&id);
    }

    /// Routes a completion to its waiter. Unknown ids (timed out, duplicate
    /// completion) are dropped silently — the outcome may hold key material
    /// and must not be parked anywhere.
    pub fn complete_waiter(id: i64, outcome: RetrieveOutcome) {
        let Some(sender) = lock_waiters().remove(&id) else {
            return;
        };
        // A disconnected receiver means the caller gave up; nothing to do.
        let _ = sender.send(outcome);
    }

    /// Maps a status (+ payload for `STATUS_OK`) to the trait outcome. Used
    /// for both the synchronous return of `retrieve` and the async
    /// completion; messages carry numeric codes only, never content.
    pub fn outcome(status: i32, secret: Option<Vec<u8>>) -> RetrieveOutcome {
        match status {
            STATUS_OK => match secret {
                Some(bytes) => Ok(Some(Zeroizing::new(bytes))),
                None => Err(SecureStoreError::Backend(
                    "completion missing payload".to_string(),
                )),
            },
            STATUS_NOT_FOUND => Ok(None),
            STATUS_UNAVAILABLE => Err(SecureStoreError::Unavailable),
            STATUS_DENIED => Err(SecureStoreError::AccessDenied),
            other => Err(SecureStoreError::Backend(format!(
                "secure store status {other}"
            ))),
        }
    }
}

#[cfg(target_os = "android")]
pub(crate) mod android {
    //! Android Keystore backend. Scheme decision (recorded here, mirroring
    //! the iOS decision above):
    //!
    //! - The MK copy is wrapped by a Keystore-resident RSA-2048 keypair whose
    //!   private key is non-extractable and demands a per-use STRONG
    //!   biometric (`setUserAuthenticationRequired`). The wrapped blob lives
    //!   as a private app file — Keystore stores keys, never data. Decrypting
    //!   the blob IS the biometric gate (iOS userPresence parity: the read
    //!   presents the sheet), enforced by binding the decrypt `Cipher` into
    //!   the prompt's `CryptoObject`.
    //! - RSA (asymmetric) instead of the more obvious Keystore AES key:
    //!   an auth-required AES key gates encryption too, which would force a
    //!   prompt inside `store()` — iOS stores silently, and `store()` must
    //!   stay prompt-free. Public-key wrapping needs no authentication.
    //! - No new prompt plugin: `tauri-plugin-biometric` only authenticates,
    //!   it does not gate the stored key (the iOS rationale, unchanged). No
    //!   androidx.biometric either: the framework BiometricPrompt is API 28+
    //!   which equals minSdk 28, so the library would only add backports
    //!   below our floor. The only build change is the `jni` crate — already
    //!   in the tree via tao/wry at the same version.
    //!
    //! Rust↔Kotlin handshake (Kotlin half in TypviaSecureStore.kt):
    //!
    //! 1. One-time capture: the first store call runs a closure on the main
    //!    pipe (`with_webview` → `jni_handle().exec`) that hands back the
    //!    `JavaVM` and a global ref to the activity. Later calls attach the
    //!    current blocking thread and call the MainActivity bridge methods
    //!    directly — object methods resolve through the instance, so the
    //!    native-thread class-loader limitation never applies, and no work
    //!    is funnelled through the main thread.
    //! 2. `store`/`remove` are synchronous Kotlin calls on the blocking
    //!    thread (Keystore and file I/O are thread-safe).
    //! 3. `retrieve` registers a waiter id, calls the Kotlin side, which
    //!    posts the prompt to the main looper and returns `PENDING`; the
    //!    blocking thread then parks on the waiter channel with a timeout
    //!    (no busy-wait, no indefinite block). The prompt outcome comes back
    //!    through the `nativeSecureStoreComplete` JNI export and is routed
    //!    by request id; late or duplicate completions are dropped.
    //!
    //! Deadlock discipline: the commands layer guarantees every secure-store
    //! call happens on a blocking-pool thread and that no DB-connection lock
    //! is held while the prompt is up (commands.rs, vault section).

    use std::sync::OnceLock;
    use std::sync::mpsc::sync_channel;
    use std::time::Duration;

    use jni::objects::{GlobalRef, JByteArray, JClass, JObject};
    use jni::sys::{jint, jlong};
    use jni::{JNIEnv, JavaVM};
    use tauri::Manager;
    use zeroize::Zeroizing;

    use typvia_core::vault::{SecureStore, SecureStoreError};

    use super::android_protocol as protocol;

    /// One-time JVM/activity capture bound: the main pipe answers within
    /// milliseconds when the UI thread is healthy.
    const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

    /// How long a gated read waits for the user to answer the system prompt
    /// before the unlock attempt fails honestly (the sheet itself stays
    /// OS-managed; a late completion is dropped).
    const PROMPT_TIMEOUT: Duration = Duration::from_secs(90);

    /// The one-time JVM/activity capture every JNI-backed platform feature in
    /// this host shares (secure store, sensitive clipboard).
    pub(crate) struct JavaHost {
        pub(crate) vm: JavaVM,
        pub(crate) activity: GlobalRef,
    }

    static HOST: OnceLock<JavaHost> = OnceLock::new();

    /// Returns the captured JVM + activity, capturing them on first use via
    /// the main pipe (`with_webview` → `jni_handle().exec`). Runs once per
    /// process; racing first calls both produce a valid capture and the loser
    /// is simply dropped.
    pub(crate) fn java_host(app: &tauri::AppHandle) -> Result<&'static JavaHost, SecureStoreError> {
        if let Some(host) = HOST.get() {
            return Ok(host);
        }
        let webview = app
            .get_webview_window("main")
            .ok_or(SecureStoreError::Unavailable)?;
        let (sender, receiver) = sync_channel(1);
        webview
            .with_webview(move |platform_webview| {
                platform_webview
                    .jni_handle()
                    .exec(move |env, activity, _webview| {
                        let vm = env.get_java_vm().ok();
                        let activity = env.new_global_ref(activity).ok();
                        let _ = sender.send(vm.zip(activity));
                    });
            })
            .map_err(|_| SecureStoreError::Unavailable)?;
        let (vm, activity) = receiver
            .recv_timeout(CAPTURE_TIMEOUT)
            .ok()
            .flatten()
            .ok_or_else(|| SecureStoreError::Backend("jvm capture failed".to_string()))?;
        let _ = HOST.set(JavaHost { vm, activity });
        HOST.get()
            .ok_or_else(|| SecureStoreError::Backend("jvm capture lost".to_string()))
    }

    /// Android Keystore backend. Stateless beyond the app handle used for
    /// the one-time JVM capture.
    pub struct AndroidKeystore {
        app: tauri::AppHandle,
    }

    impl AndroidKeystore {
        pub fn new(app: tauri::AppHandle) -> Self {
            Self { app }
        }

        /// The shared per-process JVM/activity capture (see [`java_host`]).
        fn host(&self) -> Result<&'static JavaHost, SecureStoreError> {
            java_host(&self.app)
        }
    }

    impl SecureStore for AndroidKeystore {
        fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError> {
            let host = self.host()?;
            let mut env = attach(host)?;
            let gated = super::needs_user_presence(entry);
            let status = (|| -> jni::errors::Result<i32> {
                let entry = env.new_string(entry)?;
                let secret = env.byte_array_from_slice(secret)?;
                env.call_method(
                    host.activity.as_obj(),
                    "typviaSecureStoreStore",
                    "(Ljava/lang/String;[BZ)I",
                    &[(&entry).into(), (&secret).into(), gated.into()],
                )?
                .i()
            })();
            match checked(&mut env, status)? {
                protocol::STATUS_OK => Ok(()),
                protocol::STATUS_UNAVAILABLE => Err(SecureStoreError::Unavailable),
                other => Err(SecureStoreError::Backend(format!(
                    "secure store status {other}"
                ))),
            }
        }

        fn retrieve(&self, entry: &str) -> Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError> {
            let host = self.host()?;
            let (request_id, receiver) = protocol::register_waiter();
            let mut env = match attach(host) {
                Ok(env) => env,
                Err(error) => {
                    protocol::deregister_waiter(request_id);
                    return Err(error);
                }
            };
            let gated = super::needs_user_presence(entry);
            let status = (|| -> jni::errors::Result<i32> {
                let entry = env.new_string(entry)?;
                env.call_method(
                    host.activity.as_obj(),
                    "typviaSecureStoreRetrieve",
                    "(Ljava/lang/String;JZ)I",
                    &[
                        (&entry).into(),
                        jlong::from(request_id).into(),
                        gated.into(),
                    ],
                )?
                .i()
            })();
            settle_retrieve(&mut env, request_id, &receiver, status)
        }

        fn remove(&self, entry: &str) -> Result<(), SecureStoreError> {
            let host = self.host()?;
            let mut env = attach(host)?;
            let status = (|| -> jni::errors::Result<i32> {
                let entry = env.new_string(entry)?;
                env.call_method(
                    host.activity.as_obj(),
                    "typviaSecureStoreRemove",
                    "(Ljava/lang/String;)I",
                    &[(&entry).into()],
                )?
                .i()
            })();
            match checked(&mut env, status)? {
                protocol::STATUS_OK => Ok(()),
                other => Err(SecureStoreError::Backend(format!(
                    "secure store status {other}"
                ))),
            }
        }
    }

    pub(crate) fn attach(
        host: &'static JavaHost,
    ) -> Result<jni::AttachGuard<'static>, SecureStoreError> {
        host.vm
            .attach_current_thread()
            .map_err(|_| SecureStoreError::Backend("jvm attach failed".to_string()))
    }

    /// Shared tail of both retrieve bridges (activity and headless): maps
    /// the immediate status, parks on the waiter channel when the answer is
    /// PENDING — delivered by the main executor for a gated read (after the
    /// prompt), inline on this thread for an ungated one (the channel
    /// buffers it, so the receive returns at once) — and always deregisters
    /// on the failure paths.
    fn settle_retrieve(
        env: &mut JNIEnv<'_>,
        request_id: i64,
        receiver: &std::sync::mpsc::Receiver<protocol::RetrieveOutcome>,
        status: jni::errors::Result<i32>,
    ) -> protocol::RetrieveOutcome {
        let status = match checked(env, status) {
            Ok(status) => status,
            Err(error) => {
                protocol::deregister_waiter(request_id);
                return Err(error);
            }
        };
        if status == protocol::STATUS_PENDING {
            match receiver.recv_timeout(PROMPT_TIMEOUT) {
                Ok(outcome) => outcome,
                Err(_) => {
                    protocol::deregister_waiter(request_id);
                    Err(SecureStoreError::Backend(
                        "biometric prompt timed out".to_string(),
                    ))
                }
            }
        } else {
            protocol::deregister_waiter(request_id);
            protocol::outcome(status, None)
        }
    }

    /// Keystore bridge for the WorkManager background round: that
    /// process shape may hold no activity and no Tauri state, so calls go
    /// straight to the `TypviaSecureStore` Kotlin object with the
    /// application context. Gated entries are refused up front — a
    /// biometric prompt needs a foreground activity, and the background
    /// round only ever touches ungated sync material (K_sync, device keys;
    /// `needs_user_presence` is the single classifier both bridges share).
    pub(crate) struct HeadlessKeystore {
        vm: JavaVM,
        store_object: GlobalRef,
        context: GlobalRef,
    }

    impl HeadlessKeystore {
        /// Builds the bridge inside a Java-originated JNI frame (the
        /// worker's native call), where app classes still resolve; the kept
        /// global refs make every later call class-loader independent, so
        /// the FindClass-on-native-thread limitation never applies.
        pub(crate) fn from_worker_call(
            env: &mut JNIEnv<'_>,
            context: &JObject<'_>,
        ) -> Option<Self> {
            let vm = env.get_java_vm().ok()?;
            let class = env.find_class("dev/typvia/mobile/TypviaSecureStore").ok()?;
            let instance = env
                .get_static_field(&class, "INSTANCE", "Ldev/typvia/mobile/TypviaSecureStore;")
                .ok()?
                .l()
                .ok()?;
            let store_object = env.new_global_ref(&instance).ok()?;
            let context = env.new_global_ref(context).ok()?;
            Some(Self {
                vm,
                store_object,
                context,
            })
        }

        fn env(&self) -> Result<jni::AttachGuard<'_>, SecureStoreError> {
            self.vm
                .attach_current_thread()
                .map_err(|_| SecureStoreError::Backend("jvm attach failed".to_string()))
        }
    }

    impl SecureStore for HeadlessKeystore {
        fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError> {
            if super::needs_user_presence(entry) {
                return Err(SecureStoreError::Unavailable);
            }
            let mut env = self.env()?;
            let status = (|| -> jni::errors::Result<i32> {
                let entry = env.new_string(entry)?;
                let secret = env.byte_array_from_slice(secret)?;
                env.call_method(
                    self.store_object.as_obj(),
                    "store",
                    "(Landroid/content/Context;Ljava/lang/String;[BZ)I",
                    &[
                        (self.context.as_obj()).into(),
                        (&entry).into(),
                        (&secret).into(),
                        false.into(),
                    ],
                )?
                .i()
            })();
            match checked(&mut env, status)? {
                protocol::STATUS_OK => Ok(()),
                protocol::STATUS_UNAVAILABLE => Err(SecureStoreError::Unavailable),
                other => Err(SecureStoreError::Backend(format!(
                    "secure store status {other}"
                ))),
            }
        }

        fn retrieve(&self, entry: &str) -> Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError> {
            if super::needs_user_presence(entry) {
                return Err(SecureStoreError::Unavailable);
            }
            let (request_id, receiver) = protocol::register_waiter();
            let mut env = match self.env() {
                Ok(env) => env,
                Err(error) => {
                    protocol::deregister_waiter(request_id);
                    return Err(error);
                }
            };
            let status = (|| -> jni::errors::Result<i32> {
                let entry = env.new_string(entry)?;
                env.call_method(
                    self.store_object.as_obj(),
                    "retrieve",
                    "(Landroid/content/Context;Ljava/lang/String;JZ)I",
                    &[
                        (self.context.as_obj()).into(),
                        (&entry).into(),
                        jlong::from(request_id).into(),
                        false.into(),
                    ],
                )?
                .i()
            })();
            settle_retrieve(&mut env, request_id, &receiver, status)
        }

        fn remove(&self, entry: &str) -> Result<(), SecureStoreError> {
            let mut env = self.env()?;
            let status = (|| -> jni::errors::Result<i32> {
                let entry = env.new_string(entry)?;
                env.call_method(
                    self.store_object.as_obj(),
                    "remove",
                    "(Landroid/content/Context;Ljava/lang/String;)I",
                    &[(self.context.as_obj()).into(), (&entry).into()],
                )?
                .i()
            })();
            match checked(&mut env, status)? {
                protocol::STATUS_OK => Ok(()),
                other => Err(SecureStoreError::Backend(format!(
                    "secure store status {other}"
                ))),
            }
        }
    }

    /// Collapses a JNI-level failure into a backend error, clearing any
    /// pending Java exception so the thread stays usable. Codes only — a
    /// Java exception message could name key aliases.
    fn checked(
        env: &mut JNIEnv<'_>,
        result: jni::errors::Result<i32>,
    ) -> Result<i32, SecureStoreError> {
        match result {
            Ok(status) => Ok(status),
            Err(_) => {
                if env.exception_check().unwrap_or(false) {
                    let _ = env.exception_clear();
                }
                Err(SecureStoreError::Backend("jni call failed".to_string()))
            }
        }
    }

    /// JNI export TypviaSecureStore.kt calls from the main executor with the
    /// prompt outcome. Never panics and never logs; completions for waiters
    /// that already timed out are dropped (the buffer zeroizes on drop).
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_dev_typvia_mobile_TypviaSecureStore_nativeSecureStoreComplete<
        'local,
    >(
        env: JNIEnv<'local>,
        _class: JClass<'local>,
        request_id: jlong,
        status: jint,
        secret: JByteArray<'local>,
    ) {
        let payload = if secret.is_null() {
            None
        } else {
            match env.convert_byte_array(&secret) {
                Ok(bytes) => Some(bytes),
                Err(_) => {
                    if env.exception_check().unwrap_or(false) {
                        let _ = env.exception_clear();
                    }
                    // STATUS_OK without a readable payload maps to an honest
                    // backend error in `outcome`.
                    None
                }
            }
        };
        protocol::complete_waiter(request_id, protocol::outcome(status, payload));
    }
}

#[cfg(test)]
mod tests {
    use super::android_protocol as protocol;
    use super::needs_user_presence;
    use typvia_core::vault::{BIOMETRIC_MK_ENTRY, SecureStoreError};
    use typvia_sync::{DEVICE_ED25519_ENTRY, DEVICE_X25519_ENTRY, K_SYNC_ENTRY_PREFIX};

    /// Red line: only the vault master-key copy is gated. Gating any sync
    /// entry would raise a biometric prompt on every sync round and make sync
    /// impossible on a device with no enrolled biometric — both platform
    /// backends read this one predicate.
    #[test]
    fn only_the_vault_master_key_copy_is_gated_on_user_presence() {
        assert!(needs_user_presence(BIOMETRIC_MK_ENTRY));

        assert!(!needs_user_presence(DEVICE_ED25519_ENTRY));
        assert!(!needs_user_presence(DEVICE_X25519_ENTRY));
        for key_id in [1_u32, 2, 47] {
            assert!(!needs_user_presence(&format!(
                "{K_SYNC_ENTRY_PREFIX}{key_id}"
            )));
        }
    }

    #[test]
    fn an_unknown_entry_name_is_not_gated_by_accident() {
        assert!(!needs_user_presence("vault.mk"));
        assert!(!needs_user_presence("vault.mk.biometric.old"));
        assert!(!needs_user_presence(""));
    }

    #[test]
    fn completion_routes_the_payload_to_the_registered_waiter() {
        let (id, receiver) = protocol::register_waiter();

        protocol::complete_waiter(
            id,
            protocol::outcome(protocol::STATUS_OK, Some(vec![7; 32])),
        );

        let delivered = receiver.try_recv().unwrap().unwrap().unwrap();
        assert_eq!(delivered.as_slice(), &[7u8; 32]);
    }

    #[test]
    fn a_deregistered_waiter_drops_late_completions() {
        let (id, receiver) = protocol::register_waiter();
        protocol::deregister_waiter(id);

        protocol::complete_waiter(id, protocol::outcome(protocol::STATUS_OK, Some(vec![1])));

        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn only_the_first_completion_for_a_request_counts() {
        let (id, receiver) = protocol::register_waiter();

        protocol::complete_waiter(id, protocol::outcome(protocol::STATUS_DENIED, None));
        protocol::complete_waiter(id, protocol::outcome(protocol::STATUS_OK, Some(vec![2])));

        assert!(matches!(
            receiver.try_recv().unwrap(),
            Err(SecureStoreError::AccessDenied)
        ));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn statuses_map_to_the_secure_store_trait_semantics() {
        assert!(matches!(
            protocol::outcome(protocol::STATUS_NOT_FOUND, None),
            Ok(None)
        ));
        assert!(matches!(
            protocol::outcome(protocol::STATUS_UNAVAILABLE, None),
            Err(SecureStoreError::Unavailable)
        ));
        assert!(matches!(
            protocol::outcome(protocol::STATUS_DENIED, None),
            Err(SecureStoreError::AccessDenied)
        ));
        // OK without a payload and unknown codes are backend faults, never a
        // silent success.
        assert!(matches!(
            protocol::outcome(protocol::STATUS_OK, None),
            Err(SecureStoreError::Backend(_))
        ));
        assert!(matches!(
            protocol::outcome(protocol::STATUS_PENDING, None),
            Err(SecureStoreError::Backend(_))
        ));
        assert!(matches!(
            protocol::outcome(99, None),
            Err(SecureStoreError::Backend(_))
        ));
    }
}

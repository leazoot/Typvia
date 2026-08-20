//! Interactive live verification of the macOS Keychain SecureStore backend on
//! a real device: store, then biometric prompt, then retrieve. The `retrieve`
//! call triggers the Touch ID / passcode prompt, which cannot be automated
//! headless, so this is excluded from the default run and executed manually
//! for acceptance:
//!
//! ```sh
//! cargo test -p typvia-desktop --test keychain_live -- --ignored --nocapture
//! ```
//!
//! Preconditions: a logged-in GUI session with Touch ID or a device passcode
//! enrolled. Approve the biometric prompt when it appears.
//!
//! The stored bytes are an explicit fake — never real key material.
#![cfg(target_os = "macos")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use typvia_desktop::secure_store::{LIVE_TEST_GATED_ENTRY, platform_secure_store};

/// The dedicated gated test entry (never the real vault MK copy). It shares
/// the MK entry's userPresence gating — an ungated stand-in would silently
/// skip the very path this test exists to prove, passing for the wrong reason.
/// Requires an Apple-issued signing identity: unsigned/self-signed builds
/// fail the store with OSStatus -34018 (missing entitlement) by design.
const ENTRY: &str = LIVE_TEST_GATED_ENTRY;
/// Obvious fake, 32 bytes (the MK length) — not real key material.
const FAKE_MK: &[u8] = b"KEYCHAIN_LIVE_FAKE_MK_do_not_use";

#[test]
#[ignore = "requires a GUI session with Touch ID / passcode; run manually for acceptance"]
fn stores_gates_and_retrieves_the_mk_copy() {
    let store = platform_secure_store().expect("macOS keychain backend");

    // Clean slate: a prior run may have left the entry behind.
    store.remove(ENTRY).expect("pre-clean remove");

    store.store(ENTRY, FAKE_MK).expect("store MK copy");

    // This prompts Touch ID / passcode — approve it to proceed.
    let retrieved = store
        .retrieve(ENTRY)
        .expect("retrieve (approve the biometric prompt)")
        .expect("entry must be present after store");
    assert_eq!(
        retrieved.as_slice(),
        FAKE_MK,
        "the MK copy must round-trip byte-for-byte through the Keychain"
    );

    store.remove(ENTRY).expect("remove");
    let after = store.retrieve(ENTRY).expect("retrieve after remove");
    assert!(after.is_none(), "entry must be gone after removal");

    println!("KEYCHAIN_LIVE_RESULT store=ok gated_retrieve=ok remove=ok");
}

//! Per-device key pairs and the pairing fingerprint.
//!
//! Each device owns an Ed25519 pair (identity / payload signing) and an
//! X25519 pair (pairing key exchange). Private halves are persisted only
//! through the platform [`SecureStore`], never the database;
//! in memory they zeroize on drop and `Debug` output is redacted.

use std::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use typvia_crypto::SecureStore;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::{Zeroize, Zeroizing};

use crate::base32::encode_nopad;
use crate::error::IdentityError;

/// Secure-store entry holding the 32-byte Ed25519 seed (naming follows the
/// existing `vault.mk.biometric` entry style).
pub const DEVICE_ED25519_ENTRY: &str = "device.ed25519.seed";
/// Secure-store entry holding the 32-byte X25519 static secret.
pub const DEVICE_X25519_ENTRY: &str = "device.x25519.secret";

const SECRET_LEN: usize = 32;

/// The local device's key material. Both secrets wipe themselves on drop
/// (dalek `zeroize` feature); the type exposes only public halves and
/// signing — raw secret bytes leave it solely via [`Self::persist`].
pub struct DeviceIdentity {
    signing: SigningKey,
    exchange: StaticSecret,
}

impl DeviceIdentity {
    /// Generates a fresh identity from the OS CSPRNG.
    pub fn generate() -> Self {
        Self {
            signing: SigningKey::generate(&mut OsRng),
            exchange: StaticSecret::random_from_rng(OsRng),
        }
    }

    /// Writes both private halves to the platform secure store. Entries are
    /// replaced atomically per entry; a failure between the two writes is
    /// recoverable because [`Self::persist`] is idempotent for the same
    /// identity.
    pub fn persist(&self, store: &dyn SecureStore) -> Result<(), IdentityError> {
        let seed = Zeroizing::new(self.signing.to_bytes());
        store.store(DEVICE_ED25519_ENTRY, seed.as_ref())?;
        let secret = Zeroizing::new(self.exchange.to_bytes());
        store.store(DEVICE_X25519_ENTRY, secret.as_ref())?;
        Ok(())
    }

    /// Loads the identity back from the secure store. `Ok(None)` when no
    /// identity exists yet; a half-present or malformed identity is an
    /// error, never a silent regeneration (that would mint a second device).
    pub fn load(store: &dyn SecureStore) -> Result<Option<Self>, IdentityError> {
        let seed = store.retrieve(DEVICE_ED25519_ENTRY)?;
        let secret = store.retrieve(DEVICE_X25519_ENTRY)?;
        match (seed, secret) {
            (None, None) => Ok(None),
            (Some(seed), Some(secret)) => {
                let mut seed_bytes: [u8; SECRET_LEN] = seed
                    .as_slice()
                    .try_into()
                    .map_err(|_| IdentityError::CorruptKeyMaterial)?;
                let mut secret_bytes: [u8; SECRET_LEN] = secret
                    .as_slice()
                    .try_into()
                    .map_err(|_| IdentityError::CorruptKeyMaterial)?;
                let identity = Self {
                    signing: SigningKey::from_bytes(&seed_bytes),
                    exchange: StaticSecret::from(secret_bytes),
                };
                seed_bytes.zeroize();
                secret_bytes.zeroize();
                Ok(Some(identity))
            }
            _ => Err(IdentityError::Incomplete),
        }
    }

    /// Ed25519 public key bytes (identity anchor; goes into the device row).
    pub fn ed25519_public(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    /// X25519 public key bytes (pairing exchange; travels in certificates).
    pub fn x25519_public(&self) -> [u8; 32] {
        X25519PublicKey::from(&self.exchange).to_bytes()
    }

    /// The device fingerprint derived from the Ed25519 public key.
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of_ed25519_public(&self.ed25519_public())
    }

    /// The verifying key, for checking this device's own signatures.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    /// Signs protocol bytes with the device's Ed25519 key.
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing.sign(message)
    }

    /// X25519 shared secret with a counterpart public key (pairing key
    /// exchange). The returned secret zeroizes on drop; callers
    /// keep it strictly inside the derivation step.
    pub(crate) fn diffie_hellman(&self, their_public: &[u8; 32]) -> x25519_dalek::SharedSecret {
        self.exchange
            .diffie_hellman(&X25519PublicKey::from(*their_public))
    }
}

impl fmt::Debug for DeviceIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Secrets never reach Debug output; the fingerprint is public and
        // identifies the device for diagnostics.
        write!(f, "DeviceIdentity(<redacted>, {})", self.fingerprint())
    }
}

/// SHA-256 digest of a device's Ed25519 public key — the identity anchor.
/// The full digest compares roots; [`Self::display_code`] is the
/// human-checked pairing form.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// Fingerprint of the given Ed25519 public key bytes.
    pub fn of_ed25519_public(public_key: &[u8; 32]) -> Self {
        Self(Sha256::digest(public_key).into())
    }

    /// Rebuilds a fingerprint from its stored 32-byte digest (the pinned
    /// trust root persisted in sync_config).
    pub fn from_bytes(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    /// The full 32-byte digest, e.g. for pinning the trust root.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Display form: `base32(SHA-256(pub))` (RFC 4648, upper-case, no
    /// padding), first 20 characters in 4 hyphen-joined groups of 5.
    pub fn display_code(&self) -> String {
        // The ASCII base32 form of a 32-byte digest is 52 characters, so
        // the four 5-character groups always exist.
        let code = encode_nopad(&self.0);
        format!(
            "{}-{}-{}-{}",
            &code[0..5],
            &code[5..10],
            &code[10..15],
            &code[15..20]
        )
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display_code())
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({})", self.display_code())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use typvia_crypto::SecureStoreError;

    use super::*;

    /// In-memory secure-store double for tests (no platform I/O).
    pub(crate) struct MemoryStore {
        entries: RefCell<HashMap<String, Vec<u8>>>,
    }

    impl MemoryStore {
        pub(crate) fn new() -> Self {
            Self {
                entries: RefCell::new(HashMap::new()),
            }
        }
    }

    impl SecureStore for MemoryStore {
        fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError> {
            self.entries
                .borrow_mut()
                .insert(entry.to_string(), secret.to_vec());
            Ok(())
        }

        fn retrieve(&self, entry: &str) -> Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError> {
            Ok(self
                .entries
                .borrow()
                .get(entry)
                .cloned()
                .map(Zeroizing::new))
        }

        fn remove(&self, entry: &str) -> Result<(), SecureStoreError> {
            self.entries.borrow_mut().remove(entry);
            Ok(())
        }
    }

    #[test]
    fn generated_identities_are_distinct() {
        let a = DeviceIdentity::generate();
        let b = DeviceIdentity::generate();
        assert_ne!(a.ed25519_public(), b.ed25519_public());
        assert_ne!(a.x25519_public(), b.x25519_public());
    }

    #[test]
    fn persist_then_load_restores_the_same_identity() {
        let store = MemoryStore::new();
        let original = DeviceIdentity::generate();
        original.persist(&store).unwrap();

        let restored = DeviceIdentity::load(&store).unwrap().unwrap();
        assert_eq!(restored.ed25519_public(), original.ed25519_public());
        assert_eq!(restored.x25519_public(), original.x25519_public());

        // The restored key signs identically (same private half).
        let message = b"typvia identity roundtrip";
        assert_eq!(
            restored.sign(message).to_bytes(),
            original.sign(message).to_bytes()
        );
    }

    #[test]
    fn load_is_none_when_no_identity_exists() {
        let store = MemoryStore::new();
        assert!(DeviceIdentity::load(&store).unwrap().is_none());
    }

    #[test]
    fn a_half_present_identity_is_an_error_not_a_regeneration() {
        let store = MemoryStore::new();
        DeviceIdentity::generate().persist(&store).unwrap();
        store.remove(DEVICE_X25519_ENTRY).unwrap();

        assert!(matches!(
            DeviceIdentity::load(&store),
            Err(IdentityError::Incomplete)
        ));
    }

    #[test]
    fn a_wrong_length_entry_is_corrupt_key_material() {
        let store = MemoryStore::new();
        store.store(DEVICE_ED25519_ENTRY, &[0xAA; 16]).unwrap();
        store.store(DEVICE_X25519_ENTRY, &[0xBB; 32]).unwrap();

        assert!(matches!(
            DeviceIdentity::load(&store),
            Err(IdentityError::CorruptKeyMaterial)
        ));
    }

    #[test]
    fn debug_output_is_redacted() {
        let store = MemoryStore::new();
        let identity = DeviceIdentity::generate();
        identity.persist(&store).unwrap();

        let rendered = format!("{identity:?}");
        assert!(rendered.contains("<redacted>"));
        // No stored secret byte sequence may appear in the rendering, in
        // any common encoding of it.
        let entries = store.entries.borrow();
        for secret in entries.values() {
            let hex_lower: String = secret.iter().map(|b| format!("{b:02x}")).collect();
            let hex_upper = hex_lower.to_uppercase();
            assert!(!rendered.contains(&hex_lower));
            assert!(!rendered.contains(&hex_upper));
        }
    }

    #[test]
    fn zeroizing_buffers_wipe_on_explicit_zeroize() {
        // The persistence path moves secrets through `Zeroizing` buffers;
        // this pins the wipe behavior the path relies on.
        let mut buffer = Zeroizing::new([0xCD_u8; SECRET_LEN]);
        buffer.zeroize();
        assert_eq!(*buffer, [0u8; SECRET_LEN]);
    }

    #[test]
    fn fingerprint_is_stable_for_a_fixed_public_key() {
        // Precomputed: base32(SHA-256([0xAB; 32]))[..20] in groups of 5.
        let fingerprint = Fingerprint::of_ed25519_public(&[0xAB; 32]);
        assert_eq!(fingerprint.display_code(), "TIW3F-YR7CU-CM2BL-GAZKT");
    }

    #[test]
    fn fingerprint_format_is_four_groups_of_five_base32_characters() {
        let identity = DeviceIdentity::generate();
        let code = identity.fingerprint().display_code();
        let groups: Vec<&str> = code.split('-').collect();
        assert_eq!(groups.len(), 4);
        for group in groups {
            assert_eq!(group.len(), 5);
            assert!(
                group
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c)),
                "unexpected character in {group}"
            );
        }
    }

    #[test]
    fn fingerprint_matches_the_identity_public_key() {
        let identity = DeviceIdentity::generate();
        let expected = Fingerprint::of_ed25519_public(&identity.ed25519_public());
        assert_eq!(identity.fingerprint(), expected);
    }
}

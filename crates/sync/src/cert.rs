// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Trust-root statement and device certificates.
//!
//! The account's first device self-signs a root statement; every later
//! device carries a certificate chain from the root down (issued at pairing
//! time). Verification is client-side only — the server relays these blobs
//! but can forge none of them (it holds no private keys). The signed-byte
//! encoding follows the same style as sync records: a domain-separation prefix,
//! fixed-length fields in little-endian, variable-length fields terminated
//! by 0x00 (which they therefore must not contain).

use std::collections::HashMap;

use ed25519_dalek::{Signature, VerifyingKey};
use typvia_core::model::{DeviceId, Platform, TimestampMs};

use crate::error::CertificateError;
use crate::identity::{DeviceIdentity, Fingerprint};

const ROOT_CONTEXT: &[u8] = b"typvia.root.v1";
const CERT_CONTEXT: &[u8] = b"typvia.devcert.v1";

/// The device description covered by a root statement or certificate
/// signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateSubject {
    pub device_id: DeviceId,
    /// Ed25519 public key (identity / signing).
    pub ed25519_pub: [u8; 32],
    /// X25519 public key (pairing exchange).
    pub x25519_pub: [u8; 32],
    pub name: String,
    pub platform: Platform,
    pub created_at: TimestampMs,
}

/// Self-signed root statement of the account's first device. Its
/// Ed25519 key fingerprint is the account's trust anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootStatement {
    pub subject: CertificateSubject,
    pub signature: [u8; 64],
}

impl RootStatement {
    /// Self-signs a root statement. The subject's keys must be the signing
    /// identity's own keys — a root statement asserts "I am this device".
    pub fn create(
        identity: &DeviceIdentity,
        subject: CertificateSubject,
    ) -> Result<Self, CertificateError> {
        if subject.ed25519_pub != identity.ed25519_public()
            || subject.x25519_pub != identity.x25519_public()
        {
            return Err(CertificateError::SubjectKeyMismatch);
        }
        let bytes = root_signed_bytes(&subject)?;
        let signature = identity.sign(&bytes).to_bytes();
        Ok(Self { subject, signature })
    }

    /// Verifies the self-signature.
    pub fn verify(&self) -> Result<(), CertificateError> {
        let bytes = root_signed_bytes(&self.subject)?;
        verify_signature(&self.subject.ed25519_pub, &bytes, &self.signature)
    }

    /// The account trust anchor.
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of_ed25519_public(&self.subject.ed25519_pub)
    }
}

/// A pairing-time certificate: an already-trusted device vouching for a new
/// device's keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCertificate {
    pub subject: CertificateSubject,
    /// Device that signed this certificate.
    pub issuer_device_id: DeviceId,
    /// Signing time; certificates issued at or after the issuer's
    /// revocation time are invalid.
    pub issued_at: TimestampMs,
    pub signature: [u8; 64],
}

impl DeviceCertificate {
    /// Signs a certificate for `subject` with the issuer's identity.
    pub fn issue(
        issuer: &DeviceIdentity,
        issuer_device_id: &str,
        subject: CertificateSubject,
        issued_at: TimestampMs,
    ) -> Result<Self, CertificateError> {
        let bytes = cert_signed_bytes(&subject, issuer_device_id, issued_at)?;
        let signature = issuer.sign(&bytes).to_bytes();
        Ok(Self {
            subject,
            issuer_device_id: issuer_device_id.to_string(),
            issued_at,
            signature,
        })
    }
}

/// Verifies a certificate chain down from the trust root.
///
/// `chain` runs root-first: `chain[0]` is issued by the root device and
/// each later certificate by the subject of its predecessor. `revoked_at`
/// maps revoked device ids to their revocation time — a certificate issued
/// at or after its issuer's revocation is rejected. The root device itself
/// carries no chain, so an empty chain is an error, and a root statement
/// that does not match the pinned `known_root` fingerprint is a hard
/// failure (unknown root).
pub fn verify_certificate_chain(
    known_root: &Fingerprint,
    root: &RootStatement,
    chain: &[DeviceCertificate],
    revoked_at: &HashMap<DeviceId, TimestampMs>,
) -> Result<(), CertificateError> {
    if root.fingerprint() != *known_root {
        return Err(CertificateError::UnknownRoot);
    }
    root.verify()?;
    if chain.is_empty() {
        return Err(CertificateError::EmptyChain);
    }

    let mut issuer_id: &str = &root.subject.device_id;
    let mut issuer_key: &[u8; 32] = &root.subject.ed25519_pub;
    for cert in chain {
        if cert.issuer_device_id != issuer_id {
            return Err(CertificateError::BrokenChain);
        }
        if let Some(revoked) = revoked_at.get(&cert.issuer_device_id)
            && cert.issued_at >= *revoked
        {
            return Err(CertificateError::IssuerRevoked);
        }
        let bytes = cert_signed_bytes(&cert.subject, &cert.issuer_device_id, cert.issued_at)?;
        verify_signature(issuer_key, &bytes, &cert.signature)?;
        issuer_id = &cert.subject.device_id;
        issuer_key = &cert.subject.ed25519_pub;
    }
    Ok(())
}

/// Strict Ed25519 verification (rejects malleable signatures and weak
/// public keys).
fn verify_signature(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CertificateError> {
    let key = VerifyingKey::from_bytes(public_key).map_err(|_| CertificateError::MalformedKey)?;
    key.verify_strict(message, &Signature::from_bytes(signature))
        .map_err(|_| CertificateError::BadSignature)
}

/// `"typvia.root.v1" || subject fields`.
fn root_signed_bytes(subject: &CertificateSubject) -> Result<Vec<u8>, CertificateError> {
    let mut bytes = Vec::with_capacity(128);
    bytes.extend_from_slice(ROOT_CONTEXT);
    push_subject(&mut bytes, subject)?;
    Ok(bytes)
}

/// `"typvia.devcert.v1" || subject fields || issued_at || issuer`.
fn cert_signed_bytes(
    subject: &CertificateSubject,
    issuer_device_id: &str,
    issued_at: TimestampMs,
) -> Result<Vec<u8>, CertificateError> {
    let mut bytes = Vec::with_capacity(160);
    bytes.extend_from_slice(CERT_CONTEXT);
    push_subject(&mut bytes, subject)?;
    bytes.extend_from_slice(&issued_at.to_le_bytes());
    push_str(&mut bytes, issuer_device_id)?;
    Ok(bytes)
}

fn push_subject(bytes: &mut Vec<u8>, subject: &CertificateSubject) -> Result<(), CertificateError> {
    push_str(bytes, &subject.device_id)?;
    bytes.extend_from_slice(&subject.ed25519_pub);
    bytes.extend_from_slice(&subject.x25519_pub);
    push_str(bytes, &subject.name)?;
    push_str(bytes, subject.platform.as_str())?;
    bytes.extend_from_slice(&subject.created_at.to_le_bytes());
    Ok(())
}

/// Appends a variable-length field with its 0x00 terminator. Fields with an
/// interior NUL would make the concatenation ambiguous, so they are
/// rejected outright.
fn push_str(bytes: &mut Vec<u8>, value: &str) -> Result<(), CertificateError> {
    if value.as_bytes().contains(&0) {
        return Err(CertificateError::FieldContainsNul);
    }
    bytes.extend_from_slice(value.as_bytes());
    bytes.push(0);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    struct TestDevice {
        identity: DeviceIdentity,
        subject: CertificateSubject,
    }

    fn device(id: &str, name: &str) -> TestDevice {
        let identity = DeviceIdentity::generate();
        let subject = CertificateSubject {
            device_id: id.to_string(),
            ed25519_pub: identity.ed25519_public(),
            x25519_pub: identity.x25519_public(),
            name: name.to_string(),
            platform: Platform::Macos,
            created_at: 1_700_000_000_000,
        };
        TestDevice { identity, subject }
    }

    /// Root device plus a two-link chain: root → laptop → phone.
    fn account() -> (TestDevice, RootStatement, Vec<DeviceCertificate>) {
        let root_device = device("root-dev", "First Mac");
        let laptop = device("laptop-dev", "Work laptop");
        let phone = device("phone-dev", "Phone");

        let root =
            RootStatement::create(&root_device.identity, root_device.subject.clone()).unwrap();
        let cert_laptop = DeviceCertificate::issue(
            &root_device.identity,
            "root-dev",
            laptop.subject.clone(),
            1_700_000_100_000,
        )
        .unwrap();
        let cert_phone = DeviceCertificate::issue(
            &laptop.identity,
            "laptop-dev",
            phone.subject.clone(),
            1_700_000_200_000,
        )
        .unwrap();
        (root_device, root, vec![cert_laptop, cert_phone])
    }

    fn no_revocations() -> HashMap<DeviceId, TimestampMs> {
        HashMap::new()
    }

    #[test]
    fn a_valid_chain_verifies_to_the_root() {
        let (_, root, chain) = account();
        let anchor = root.fingerprint();
        assert_eq!(
            verify_certificate_chain(&anchor, &root, &chain, &no_revocations()),
            Ok(())
        );
    }

    #[test]
    fn a_single_link_chain_issued_by_the_root_verifies() {
        let (root_device, root, _) = account();
        let phone = device("phone-2", "Second phone");
        let cert = DeviceCertificate::issue(
            &root_device.identity,
            "root-dev",
            phone.subject,
            1_700_000_300_000,
        )
        .unwrap();
        assert_eq!(
            verify_certificate_chain(&root.fingerprint(), &root, &[cert], &no_revocations()),
            Ok(())
        );
    }

    #[test]
    fn a_tampered_certificate_signature_is_rejected() {
        let (_, root, mut chain) = account();
        chain[1].signature[0] ^= 0x01;
        assert_eq!(
            verify_certificate_chain(&root.fingerprint(), &root, &chain, &no_revocations()),
            Err(CertificateError::BadSignature)
        );
    }

    #[test]
    fn a_tampered_subject_field_breaks_the_signature() {
        let (_, root, mut chain) = account();
        chain[0].subject.name = "Impostor".to_string();
        assert_eq!(
            verify_certificate_chain(&root.fingerprint(), &root, &chain, &no_revocations()),
            Err(CertificateError::BadSignature)
        );
    }

    #[test]
    fn a_forged_root_is_an_unknown_root_hard_failure() {
        let (_, real_root, chain) = account();
        // The attacker self-signs their own (valid) root statement; it
        // still cannot match the fingerprint pinned on this device.
        let attacker = device("evil-root", "Evil root");
        let forged = RootStatement::create(&attacker.identity, attacker.subject).unwrap();
        assert_eq!(
            verify_certificate_chain(&real_root.fingerprint(), &forged, &chain, &no_revocations()),
            Err(CertificateError::UnknownRoot)
        );
    }

    #[test]
    fn a_root_statement_with_a_bad_self_signature_is_rejected() {
        let (_, mut root, chain) = account();
        root.signature[10] ^= 0xFF;
        assert_eq!(
            verify_certificate_chain(&root.fingerprint(), &root, &chain, &no_revocations()),
            Err(CertificateError::BadSignature)
        );
    }

    #[test]
    fn a_gap_in_the_chain_is_rejected() {
        let (_, root, chain) = account();
        // Drop the middle certificate: the phone cert's issuer is no
        // longer the preceding link.
        let broken = vec![chain[1].clone()];
        assert_eq!(
            verify_certificate_chain(&root.fingerprint(), &root, &broken, &no_revocations()),
            Err(CertificateError::BrokenChain)
        );
    }

    #[test]
    fn a_reordered_chain_is_rejected() {
        let (_, root, mut chain) = account();
        chain.swap(0, 1);
        assert_eq!(
            verify_certificate_chain(&root.fingerprint(), &root, &chain, &no_revocations()),
            Err(CertificateError::BrokenChain)
        );
    }

    #[test]
    fn an_empty_chain_is_rejected() {
        let (_, root, _) = account();
        assert_eq!(
            verify_certificate_chain(&root.fingerprint(), &root, &[], &no_revocations()),
            Err(CertificateError::EmptyChain)
        );
    }

    #[test]
    fn a_certificate_issued_after_the_issuer_was_revoked_is_rejected() {
        let (_, root, chain) = account();
        // The laptop (issuer of the phone cert, issued_at 1_700_000_200_000)
        // was revoked before that signing time.
        let mut revocations = no_revocations();
        revocations.insert("laptop-dev".to_string(), 1_700_000_150_000);
        assert_eq!(
            verify_certificate_chain(&root.fingerprint(), &root, &chain, &revocations),
            Err(CertificateError::IssuerRevoked)
        );
    }

    #[test]
    fn a_certificate_issued_exactly_at_revocation_time_is_rejected() {
        let (_, root, chain) = account();
        let mut revocations = no_revocations();
        revocations.insert("laptop-dev".to_string(), 1_700_000_200_000);
        assert_eq!(
            verify_certificate_chain(&root.fingerprint(), &root, &chain, &revocations),
            Err(CertificateError::IssuerRevoked)
        );
    }

    #[test]
    fn a_certificate_issued_before_the_issuer_revocation_still_verifies() {
        let (_, root, chain) = account();
        // Revoked after signing: history stays valid — records and
        // certificates produced before revocation were legitimate.
        let mut revocations = no_revocations();
        revocations.insert("laptop-dev".to_string(), 1_700_000_250_000);
        assert_eq!(
            verify_certificate_chain(&root.fingerprint(), &root, &chain, &revocations),
            Ok(())
        );
    }

    #[test]
    fn a_root_statement_for_foreign_keys_is_rejected_at_creation() {
        let signer = device("a", "A");
        let other = device("b", "B");
        assert_eq!(
            RootStatement::create(&signer.identity, other.subject).unwrap_err(),
            CertificateError::SubjectKeyMismatch
        );
    }

    #[test]
    fn a_name_with_an_interior_nul_byte_is_rejected() {
        let issuer = device("root-dev", "Root");
        let mut subject = device("n", "New device").subject;
        subject.name = "evil\0name".to_string();
        assert_eq!(
            DeviceCertificate::issue(&issuer.identity, "root-dev", subject, 1).unwrap_err(),
            CertificateError::FieldContainsNul
        );
    }

    #[test]
    fn root_and_certificate_encodings_are_domain_separated() {
        // The same subject signed as a root statement must not verify as a
        // certificate signature, and vice versa (context prefixes differ).
        let root_device = device("root-dev", "First Mac");
        let root =
            RootStatement::create(&root_device.identity, root_device.subject.clone()).unwrap();
        let as_cert = DeviceCertificate {
            subject: root_device.subject.clone(),
            issuer_device_id: "root-dev".to_string(),
            issued_at: root_device.subject.created_at,
            signature: root.signature,
        };
        let bytes = cert_signed_bytes(
            &as_cert.subject,
            &as_cert.issuer_device_id,
            as_cert.issued_at,
        )
        .unwrap();
        assert_eq!(
            verify_signature(&root_device.subject.ed25519_pub, &bytes, &as_cert.signature),
            Err(CertificateError::BadSignature)
        );
    }
}

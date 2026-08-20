//! Device-directory wire codec and trust assembly.
//!
//! The server relays root statements and certificate chains as opaque
//! JSON; this module owns that client-side encoding (byte fields are
//! base64, field names mirror the server's root-statement document) and
//! rebuilds the receiver trust state that `verify_and_open` consumes.
//! All verification stays in the certificate layer — this module only
//! parses and pins.

use std::collections::HashMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};
use typvia_core::model::{DeviceId, TimestampMs};

use crate::cert::{CertificateSubject, DeviceCertificate, RootStatement};
use crate::error::CertificateError;
use crate::identity::Fingerprint;
use crate::record::TrustContext;
use crate::transport::DeviceDirectory;

/// JSON form shared by root statements and certificate subjects.
#[derive(Serialize, Deserialize)]
struct SubjectDoc {
    device_id: String,
    /// Base64, 32 bytes.
    ed25519_pub: String,
    /// Base64, 32 bytes.
    x25519_pub: String,
    name: String,
    platform: String,
    created_at: TimestampMs,
}

#[derive(Serialize, Deserialize)]
struct RootStatementDoc {
    #[serde(flatten)]
    subject: SubjectDoc,
    /// Base64, 64 bytes.
    signature: String,
}

#[derive(Serialize, Deserialize)]
struct CertificateDoc {
    #[serde(flatten)]
    subject: SubjectDoc,
    issued_at: TimestampMs,
    issuer_device_id: String,
    /// Base64, 64 bytes.
    signature: String,
}

fn subject_to_doc(subject: &CertificateSubject) -> SubjectDoc {
    SubjectDoc {
        device_id: subject.device_id.clone(),
        ed25519_pub: BASE64.encode(subject.ed25519_pub),
        x25519_pub: BASE64.encode(subject.x25519_pub),
        name: subject.name.clone(),
        platform: subject.platform.as_str().to_string(),
        created_at: subject.created_at,
    }
}

fn doc_to_subject(doc: SubjectDoc) -> Result<CertificateSubject, CertificateError> {
    Ok(CertificateSubject {
        device_id: doc.device_id,
        ed25519_pub: decode_32(&doc.ed25519_pub)?,
        x25519_pub: decode_32(&doc.x25519_pub)?,
        name: doc.name,
        platform: doc
            .platform
            .parse()
            .map_err(|_| CertificateError::MalformedKey)?,
        created_at: doc.created_at,
    })
}

fn decode_32(b64: &str) -> Result<[u8; 32], CertificateError> {
    BASE64
        .decode(b64)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(CertificateError::MalformedKey)
}

fn decode_64(b64: &str) -> Result<[u8; 64], CertificateError> {
    BASE64
        .decode(b64)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(CertificateError::BadSignature)
}

/// Serializes a root statement to the directory JSON form (also the
/// `POST /v1/accounts` body shape).
pub fn root_statement_to_json(root: &RootStatement) -> Result<Vec<u8>, CertificateError> {
    serde_json::to_vec(&RootStatementDoc {
        subject: subject_to_doc(&root.subject),
        signature: BASE64.encode(root.signature),
    })
    .map_err(|_| CertificateError::MalformedKey)
}

/// Parses a directory root statement (signature verification happens in
/// the certificate layer, not here).
pub fn root_statement_from_json(json: &[u8]) -> Result<RootStatement, CertificateError> {
    let doc: RootStatementDoc =
        serde_json::from_slice(json).map_err(|_| CertificateError::MalformedKey)?;
    Ok(RootStatement {
        subject: doc_to_subject(doc.subject)?,
        signature: decode_64(&doc.signature)?,
    })
}

/// Serializes a certificate chain (root-first) to directory JSON.
pub fn cert_chain_to_json(chain: &[DeviceCertificate]) -> Result<Vec<u8>, CertificateError> {
    let docs: Vec<CertificateDoc> = chain
        .iter()
        .map(|cert| CertificateDoc {
            subject: subject_to_doc(&cert.subject),
            issued_at: cert.issued_at,
            issuer_device_id: cert.issuer_device_id.clone(),
            signature: BASE64.encode(cert.signature),
        })
        .collect();
    serde_json::to_vec(&docs).map_err(|_| CertificateError::MalformedKey)
}

/// Serializes one certificate to its directory JSON object (the
/// `POST /v1/pair/{session}/offer` certificate body).
pub(crate) fn certificate_to_json(cert: &DeviceCertificate) -> Result<Vec<u8>, CertificateError> {
    serde_json::to_vec(&CertificateDoc {
        subject: subject_to_doc(&cert.subject),
        issued_at: cert.issued_at,
        issuer_device_id: cert.issuer_device_id.clone(),
        signature: BASE64.encode(cert.signature),
    })
    .map_err(|_| CertificateError::MalformedKey)
}

/// Parses a certificate chain from directory JSON.
pub fn cert_chain_from_json(json: &[u8]) -> Result<Vec<DeviceCertificate>, CertificateError> {
    let docs: Vec<CertificateDoc> =
        serde_json::from_slice(json).map_err(|_| CertificateError::MalformedKey)?;
    docs.into_iter()
        .map(|doc| {
            Ok(DeviceCertificate {
                subject: doc_to_subject(doc.subject)?,
                issuer_device_id: doc.issuer_device_id,
                issued_at: doc.issued_at,
                signature: decode_64(&doc.signature)?,
            })
        })
        .collect()
}

/// Owned receiver trust state rebuilt from a pulled directory. The pinned
/// root check happens on construction — a mismatch is a hard failure —
/// and again per record inside `verify_and_open`.
#[derive(Debug)]
pub struct TrustState {
    root: RootStatement,
    chains: HashMap<DeviceId, Vec<DeviceCertificate>>,
    revoked_at: HashMap<DeviceId, TimestampMs>,
}

impl TrustState {
    /// Parses the directory and pins it against the local trust root.
    /// A directory whose root does not match the pin is a hard failure —
    /// a malicious server cannot swap the account's identity anchor.
    pub fn from_directory(
        directory: &DeviceDirectory,
        pinned: &Fingerprint,
    ) -> Result<Self, CertificateError> {
        let root = root_statement_from_json(&directory.root_statement_json)?;
        if root.fingerprint() != *pinned {
            return Err(CertificateError::UnknownRoot);
        }
        Self::with_root(root, directory)
    }

    /// Builds a trust state around an explicitly supplied root statement
    /// instead of the directory's. Serves the one-time recovery catch-up
    /// flow: the predecessor root travels inside the sealed recovery
    /// bundle — authenticated by recovery-code possession — and lets the
    /// recovering device verify the historical records whose chains end at
    /// the old root the directory no longer carries.
    pub(crate) fn with_root(
        root: RootStatement,
        directory: &DeviceDirectory,
    ) -> Result<Self, CertificateError> {
        root.verify()?;

        let mut chains = HashMap::new();
        let mut revoked_at: HashMap<DeviceId, TimestampMs> = HashMap::new();
        for device in &directory.devices {
            if device.id != root.subject.device_id {
                chains.insert(
                    device.id.clone(),
                    cert_chain_from_json(&device.cert_chain_json)?,
                );
            }
            if let Some(revoked) = device.revoked_at {
                revoked_at.insert(device.id.clone(), revoked);
            }
        }
        for revocation in &directory.revocations {
            let earliest = revoked_at
                .get(&revocation.device_id)
                .copied()
                .map_or(revocation.revoked_at, |t| t.min(revocation.revoked_at));
            revoked_at.insert(revocation.device_id.clone(), earliest);
        }
        Ok(Self {
            root,
            chains,
            revoked_at,
        })
    }

    /// Borrows the state as the `verify_and_open` trust context.
    pub fn context<'a>(&'a self, pinned: &'a Fingerprint) -> TrustContext<'a> {
        TrustContext {
            known_root: pinned,
            root: &self.root,
            chains: &self.chains,
            revoked_at: &self.revoked_at,
        }
    }

    /// The verified account root statement.
    pub(crate) fn root(&self) -> &RootStatement {
        &self.root
    }

    /// The revocation time of a device, if any.
    pub(crate) fn revoked_time(&self, device_id: &str) -> Option<TimestampMs> {
        self.revoked_at.get(device_id).copied()
    }

    /// The verified `(ed25519_pub, x25519_pub)` of a device: the root's
    /// keys come from its verified statement, everything else requires a
    /// chain that verifies to the pinned root and ends at the device. Key
    /// material is taken from the signed certificate subject, never from
    /// unauthenticated directory rows — a malicious server cannot swap in
    /// its own exchange key.
    pub(crate) fn verified_device_keys(
        &self,
        pinned: &Fingerprint,
        device_id: &str,
    ) -> Option<([u8; 32], [u8; 32])> {
        if device_id == self.root.subject.device_id {
            return Some((self.root.subject.ed25519_pub, self.root.subject.x25519_pub));
        }
        let chain = self.chains.get(device_id)?;
        crate::cert::verify_certificate_chain(pinned, &self.root, chain, &self.revoked_at).ok()?;
        let leaf = chain.last()?;
        (leaf.subject.device_id == device_id)
            .then_some((leaf.subject.ed25519_pub, leaf.subject.x25519_pub))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use typvia_core::model::Platform;

    use super::*;
    use crate::identity::DeviceIdentity;
    use crate::transport::{DirectoryDevice, DirectoryRevocation};

    fn subject(id: &str, identity: &DeviceIdentity) -> CertificateSubject {
        CertificateSubject {
            device_id: id.to_string(),
            ed25519_pub: identity.ed25519_public(),
            x25519_pub: identity.x25519_public(),
            name: format!("Device {id}"),
            platform: Platform::Macos,
            created_at: 1_700_000_000_000,
        }
    }

    #[test]
    fn root_statement_json_round_trips_and_still_verifies() {
        let identity = DeviceIdentity::generate();
        let root = RootStatement::create(&identity, subject("root-dev", &identity)).unwrap();
        let json = root_statement_to_json(&root).unwrap();
        let parsed = root_statement_from_json(&json).unwrap();
        assert_eq!(parsed, root);
        parsed.verify().unwrap();
    }

    #[test]
    fn cert_chain_json_round_trips() {
        let root_identity = DeviceIdentity::generate();
        let leaf = DeviceIdentity::generate();
        let cert = DeviceCertificate::issue(
            &root_identity,
            "root-dev",
            subject("leaf-dev", &leaf),
            1_700_000_100_000,
        )
        .unwrap();
        let json = cert_chain_to_json(std::slice::from_ref(&cert)).unwrap();
        let parsed = cert_chain_from_json(&json).unwrap();
        assert_eq!(parsed, vec![cert]);
    }

    #[test]
    fn trust_state_pins_the_root_and_collects_chains() {
        let root_identity = DeviceIdentity::generate();
        let root =
            RootStatement::create(&root_identity, subject("root-dev", &root_identity)).unwrap();
        let leaf = DeviceIdentity::generate();
        let cert = DeviceCertificate::issue(
            &root_identity,
            "root-dev",
            subject("leaf-dev", &leaf),
            1_700_000_100_000,
        )
        .unwrap();

        let directory = DeviceDirectory {
            root_statement_json: root_statement_to_json(&root).unwrap(),
            devices: vec![DirectoryDevice {
                id: "leaf-dev".to_string(),
                name: "Leaf".to_string(),
                platform: "macos".to_string(),
                ed25519_pub: leaf.ed25519_public().to_vec(),
                x25519_pub: leaf.x25519_public().to_vec(),
                cert_chain_json: cert_chain_to_json(std::slice::from_ref(&cert)).unwrap(),
                created_at: 1,
                revoked_at: None,
            }],
            revocations: vec![DirectoryRevocation {
                device_id: "old-dev".to_string(),
                revoked_at: 9_000,
            }],
        };

        let pinned = root.fingerprint();
        let state = TrustState::from_directory(&directory, &pinned).unwrap();
        let context = state.context(&pinned);
        assert!(context.chains.contains_key("leaf-dev"));
        assert_eq!(context.revoked_at.get("old-dev"), Some(&9_000));
    }

    #[test]
    fn a_directory_with_a_foreign_root_is_a_hard_failure() {
        let real = DeviceIdentity::generate();
        let attacker = DeviceIdentity::generate();
        let forged = RootStatement::create(&attacker, subject("root-dev", &attacker)).unwrap();
        let directory = DeviceDirectory {
            root_statement_json: root_statement_to_json(&forged).unwrap(),
            devices: Vec::new(),
            revocations: Vec::new(),
        };
        let pinned = Fingerprint::of_ed25519_public(&real.ed25519_public());
        assert_eq!(
            TrustState::from_directory(&directory, &pinned).unwrap_err(),
            CertificateError::UnknownRoot
        );
    }
}

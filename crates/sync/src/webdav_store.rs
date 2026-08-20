//! Typed I/O over the WebDAV file layout: the account marker,
//! the signed device directory, per-device record streams and their head
//! counters. This module only moves bytes into their protocol shapes —
//! verification (signatures, chains, replay) stays with the engine, which
//! treats everything read here as untrusted input.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};

use crate::record::WireRecord;
use crate::transport::{
    DeviceDirectory, DirectoryDevice, DirectoryRevocation, PulledRecord, TransportError,
};
use crate::webdav::{PutOutcome, WebdavClient};

/// Root collection under the user's base URL.
const BASE: &str = "typvia-sync";
/// File-format markers, checked on every read (untrusted input).
const ACCOUNT_FORMAT: &str = "typvia.webdav.v1";
const DIRECTORY_FORMAT: &str = "typvia.webdav.dir.v1";

/// The account marker file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebdavAccount {
    format: String,
    pub account_id: String,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize)]
struct HeadDoc {
    next_seq: u64,
}

#[derive(Serialize, Deserialize)]
struct DirectoryDoc {
    format: String,
    /// base64 of the root statement JSON.
    root_statement: String,
    devices: Vec<DirectoryDeviceDoc>,
    revocations: Vec<DirectoryRevocationDoc>,
}

#[derive(Serialize, Deserialize)]
struct DirectoryDeviceDoc {
    id: String,
    name: String,
    platform: String,
    ed25519_pub: String,
    x25519_pub: String,
    cert_chain: String,
    created_at: i64,
    #[serde(default)]
    revoked_at: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct DirectoryRevocationDoc {
    device_id: String,
    revoked_at: i64,
}

/// A record file: the wire shape minus `server_seq`, which the per-device
/// file sequence replaces.
#[derive(Serialize, Deserialize)]
struct RecordFileDoc {
    id: String,
    entity_type: String,
    entity_id: String,
    version: i64,
    #[serde(default)]
    ciphertext: Option<String>,
    #[serde(default)]
    deleted_at: Option<i64>,
    updated_at: i64,
    device_id: String,
    key_id: i64,
    signature: String,
}

/// A device directory fetched together with its ETag, for the
/// conditional-update loop.
pub struct FetchedDirectory {
    pub directory: DeviceDirectory,
    pub etag: Option<String>,
}

/// Typed store over a [`WebdavClient`].
pub struct WebdavStore {
    dav: WebdavClient,
}

impl WebdavStore {
    pub fn new(dav: WebdavClient) -> Self {
        Self { dav }
    }

    pub fn client(&self) -> &WebdavClient {
        &self.dav
    }

    fn account_path() -> String {
        format!("{BASE}/account.json")
    }

    fn directory_path() -> String {
        format!("{BASE}/directory.json")
    }

    fn head_path(device_id: &str) -> String {
        format!("{BASE}/devices/{device_id}/head.json")
    }

    fn record_path(device_id: &str, seq: u64) -> String {
        format!("{BASE}/devices/{device_id}/records/{seq:010}.json")
    }

    /// Creates the base layout collections for this device (idempotent).
    pub fn ensure_device_layout(&self, device_id: &str) -> Result<(), TransportError> {
        self.dav
            .mkcol_all(&format!("{BASE}/devices/{device_id}/records"))
    }

    // ----- account marker -----

    pub fn read_account(&self) -> Result<Option<WebdavAccount>, TransportError> {
        let Some(file) = self.dav.get(&Self::account_path())? else {
            return Ok(None);
        };
        let account: WebdavAccount =
            serde_json::from_slice(&file.bytes).map_err(|_| TransportError::MalformedResponse)?;
        if account.format != ACCOUNT_FORMAT {
            return Err(TransportError::MalformedResponse);
        }
        Ok(Some(account))
    }

    /// Claims the store for a fresh account (exclusive create): exactly one
    /// device can found an account on a given endpoint; everyone else joins
    /// by pairing — never by writing over the marker.
    pub fn create_account(&self, account_id: &str, now: i64) -> Result<PutOutcome, TransportError> {
        let doc = WebdavAccount {
            format: ACCOUNT_FORMAT.to_string(),
            account_id: account_id.to_string(),
            created_at: now,
        };
        let bytes = serde_json::to_vec(&doc).map_err(|_| TransportError::MalformedResponse)?;
        self.dav.mkcol_all(BASE)?;
        self.dav.put_exclusive(&Self::account_path(), &bytes)
    }

    // ----- device directory -----

    pub fn read_directory(&self) -> Result<Option<FetchedDirectory>, TransportError> {
        let Some(file) = self.dav.get(&Self::directory_path())? else {
            return Ok(None);
        };
        let doc: DirectoryDoc =
            serde_json::from_slice(&file.bytes).map_err(|_| TransportError::MalformedResponse)?;
        if doc.format != DIRECTORY_FORMAT {
            return Err(TransportError::MalformedResponse);
        }
        Ok(Some(FetchedDirectory {
            directory: directory_from_doc(doc)?,
            etag: file.etag,
        }))
    }

    /// First write of the directory (account foundation).
    pub fn write_directory_exclusive(
        &self,
        directory: &DeviceDirectory,
    ) -> Result<PutOutcome, TransportError> {
        let bytes = directory_to_bytes(directory)?;
        self.dav.put_exclusive(&Self::directory_path(), &bytes)
    }

    /// Conditional replace for the read-merge-write loop.
    pub fn write_directory_if_match(
        &self,
        etag: &str,
        directory: &DeviceDirectory,
    ) -> Result<PutOutcome, TransportError> {
        let bytes = directory_to_bytes(directory)?;
        self.dav.put_if_match(&Self::directory_path(), etag, &bytes)
    }

    // ----- pairing mailbox -----

    fn pairing_answer_path(claim: &str) -> String {
        format!("{BASE}/pairing/{claim}/answer.json")
    }

    /// The admitting side's one-shot answer (certificate chain + sealed key
    /// bundle). Exclusive: a pairing code is single-use.
    pub fn write_pairing_answer(
        &self,
        claim: &str,
        payload: &[u8],
    ) -> Result<PutOutcome, TransportError> {
        self.dav.mkcol_all(&format!("{BASE}/pairing/{claim}"))?;
        self.dav
            .put_exclusive(&Self::pairing_answer_path(claim), payload)
    }

    pub fn read_pairing_answer(&self, claim: &str) -> Result<Option<Vec<u8>>, TransportError> {
        Ok(self
            .dav
            .get(&Self::pairing_answer_path(claim))?
            .map(|f| f.bytes))
    }

    /// Mailbox cleanup once pairing finished (idempotent).
    pub fn delete_pairing_answer(&self, claim: &str) -> Result<(), TransportError> {
        self.dav.delete(&Self::pairing_answer_path(claim))
    }

    // ----- recovery files -----

    fn recovery_blob_path() -> String {
        format!("{BASE}/recovery/blob.json")
    }

    fn rootproof_path() -> String {
        format!("{BASE}/recovery/rootproof.json")
    }

    /// Publishes (or replaces) the recovery blob and rootproof public key.
    /// Overwrite semantics are the contract: re-publishing invalidates
    /// the previous code.
    pub fn write_recovery(
        &self,
        blob: &[u8],
        rootproof_pub: &[u8; 32],
    ) -> Result<(), TransportError> {
        self.dav.mkcol_all(&format!("{BASE}/recovery"))?;
        self.dav.put_overwrite(&Self::recovery_blob_path(), blob)?;
        self.dav
            .put_overwrite(&Self::rootproof_path(), rootproof_pub)
    }

    pub fn read_recovery_blob(&self) -> Result<Option<Vec<u8>>, TransportError> {
        Ok(self.dav.get(&Self::recovery_blob_path())?.map(|f| f.bytes))
    }

    /// Replaces the directory unconditionally — only the re-root uses
    /// this (the new root cannot know the old ETag chain, and old devices
    /// self-refuse on the pin regardless of what the file says).
    pub fn write_directory_overwrite(
        &self,
        directory: &DeviceDirectory,
    ) -> Result<(), TransportError> {
        let bytes = directory_to_bytes(directory)?;
        self.dav.put_overwrite(&Self::directory_path(), &bytes)
    }

    // ----- key-update mailboxes -----

    fn keyupdate_dir(target: &str, sender: &str) -> String {
        format!("{BASE}/keyupdates/{target}/{sender}")
    }

    fn keyupdate_head_path(target: &str, sender: &str) -> String {
        format!("{}/head.json", Self::keyupdate_dir(target, sender))
    }

    fn keyupdate_path(target: &str, sender: &str, seq: u64) -> String {
        format!("{}/{seq:010}.json", Self::keyupdate_dir(target, sender))
    }

    /// The sender-side head of one mailbox (this sender toward `target`).
    pub fn read_key_update_head(&self, target: &str, sender: &str) -> Result<u64, TransportError> {
        let Some(file) = self.dav.get(&Self::keyupdate_head_path(target, sender))? else {
            return Ok(1);
        };
        let head: HeadDoc =
            serde_json::from_slice(&file.bytes).map_err(|_| TransportError::MalformedResponse)?;
        Ok(head.next_seq.max(1))
    }

    /// Delivers one sealed key-update payload into `target`'s mailbox from
    /// this sender (single-writer directory; sequence self-heals on
    /// collision like the record stream).
    pub fn publish_key_update(
        &self,
        target: &str,
        sender: &str,
        payload: &[u8],
    ) -> Result<(), TransportError> {
        self.dav.mkcol_all(&Self::keyupdate_dir(target, sender))?;
        let mut seq = self.read_key_update_head(target, sender)?;
        loop {
            match self
                .dav
                .put_exclusive(&Self::keyupdate_path(target, sender, seq), payload)?
            {
                PutOutcome::Done => break,
                PutOutcome::PreconditionFailed => seq = seq.saturating_add(1),
            }
        }
        let bytes = serde_json::to_vec(&HeadDoc {
            next_seq: seq.saturating_add(1),
        })
        .map_err(|_| TransportError::MalformedResponse)?;
        self.dav
            .put_overwrite(&Self::keyupdate_head_path(target, sender), &bytes)
    }

    /// Reads one mailbox message; `None` when consumed or never written.
    pub fn read_key_update(
        &self,
        target: &str,
        sender: &str,
        seq: u64,
    ) -> Result<Option<Vec<u8>>, TransportError> {
        Ok(self
            .dav
            .get(&Self::keyupdate_path(target, sender, seq))?
            .map(|f| f.bytes))
    }

    /// Consumption is deletion: the receiver clears its own mailbox.
    pub fn delete_key_update(
        &self,
        target: &str,
        sender: &str,
        seq: u64,
    ) -> Result<(), TransportError> {
        self.dav.delete(&Self::keyupdate_path(target, sender, seq))
    }

    // ----- record streams -----

    /// The published head of a device's record stream: the next sequence it
    /// will write. 1 when the device has published nothing (or its head file
    /// has not appeared yet).
    pub fn read_head(&self, device_id: &str) -> Result<u64, TransportError> {
        let Some(file) = self.dav.get(&Self::head_path(device_id))? else {
            return Ok(1);
        };
        let head: HeadDoc =
            serde_json::from_slice(&file.bytes).map_err(|_| TransportError::MalformedResponse)?;
        Ok(head.next_seq.max(1))
    }

    /// Publishes this device's own head counter (single-writer overwrite).
    pub fn write_head(&self, device_id: &str, next_seq: u64) -> Result<(), TransportError> {
        let bytes = serde_json::to_vec(&HeadDoc { next_seq })
            .map_err(|_| TransportError::MalformedResponse)?;
        self.dav.put_overwrite(&Self::head_path(device_id), &bytes)
    }

    /// Publishes one sealed record under this device's stream (exclusive:
    /// a sequence collision reports back so the writer self-heals).
    pub fn publish_record(
        &self,
        device_id: &str,
        seq: u64,
        record: &WireRecord,
    ) -> Result<PutOutcome, TransportError> {
        let doc = RecordFileDoc {
            id: record.id.clone(),
            entity_type: record.entity_type.as_str().to_string(),
            entity_id: record.entity_id.clone(),
            version: i64::try_from(record.version)
                .map_err(|_| TransportError::MalformedResponse)?,
            ciphertext: if record.is_tombstone() {
                None
            } else {
                Some(BASE64.encode(&record.ciphertext))
            },
            deleted_at: record.deleted_at,
            updated_at: record.updated_at,
            device_id: record.device_id.clone(),
            key_id: i64::from(record.key_id),
            signature: BASE64.encode(record.signature),
        };
        let bytes = serde_json::to_vec(&doc).map_err(|_| TransportError::MalformedResponse)?;
        self.dav
            .put_exclusive(&Self::record_path(device_id, seq), &bytes)
    }

    /// Reads one record file from a source device's stream; `None` when the
    /// file does not exist (stream end, or a head running ahead of its
    /// files). The per-device `seq` doubles as the record's
    /// `server_seq` so downstream bookkeeping keeps one shape.
    pub fn read_record(
        &self,
        device_id: &str,
        seq: u64,
    ) -> Result<Option<PulledRecord>, TransportError> {
        let Some(file) = self.dav.get(&Self::record_path(device_id, seq))? else {
            return Ok(None);
        };
        let doc: RecordFileDoc =
            serde_json::from_slice(&file.bytes).map_err(|_| TransportError::MalformedResponse)?;
        let ciphertext = match doc.ciphertext {
            Some(b64) => BASE64
                .decode(b64)
                .map_err(|_| TransportError::MalformedResponse)?,
            None => Vec::new(),
        };
        let signature: [u8; 64] = BASE64
            .decode(doc.signature)
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(TransportError::MalformedResponse)?;
        Ok(Some(PulledRecord {
            record: WireRecord {
                id: doc.id,
                entity_type: doc
                    .entity_type
                    .parse()
                    .map_err(|_| TransportError::MalformedResponse)?,
                entity_id: doc.entity_id,
                version: u64::try_from(doc.version)
                    .map_err(|_| TransportError::MalformedResponse)?,
                ciphertext,
                deleted_at: doc.deleted_at,
                updated_at: doc.updated_at,
                device_id: doc.device_id,
                key_id: u32::try_from(doc.key_id).map_err(|_| TransportError::MalformedResponse)?,
                signature,
            },
            server_seq: seq,
        }))
    }
}

fn directory_to_bytes(directory: &DeviceDirectory) -> Result<Vec<u8>, TransportError> {
    let doc = DirectoryDoc {
        format: DIRECTORY_FORMAT.to_string(),
        root_statement: BASE64.encode(&directory.root_statement_json),
        devices: directory
            .devices
            .iter()
            .map(|d| DirectoryDeviceDoc {
                id: d.id.clone(),
                name: d.name.clone(),
                platform: d.platform.clone(),
                ed25519_pub: BASE64.encode(&d.ed25519_pub),
                x25519_pub: BASE64.encode(&d.x25519_pub),
                cert_chain: BASE64.encode(&d.cert_chain_json),
                created_at: d.created_at,
                revoked_at: d.revoked_at,
            })
            .collect(),
        revocations: directory
            .revocations
            .iter()
            .map(|r| DirectoryRevocationDoc {
                device_id: r.device_id.clone(),
                revoked_at: r.revoked_at,
            })
            .collect(),
    };
    serde_json::to_vec(&doc).map_err(|_| TransportError::MalformedResponse)
}

fn directory_from_doc(doc: DirectoryDoc) -> Result<DeviceDirectory, TransportError> {
    let decode = |b64: &str| -> Result<Vec<u8>, TransportError> {
        BASE64
            .decode(b64)
            .map_err(|_| TransportError::MalformedResponse)
    };
    Ok(DeviceDirectory {
        root_statement_json: decode(&doc.root_statement)?,
        devices: doc
            .devices
            .into_iter()
            .map(|d| {
                Ok(DirectoryDevice {
                    ed25519_pub: decode(&d.ed25519_pub)?,
                    x25519_pub: decode(&d.x25519_pub)?,
                    cert_chain_json: decode(&d.cert_chain)?,
                    id: d.id,
                    name: d.name,
                    platform: d.platform,
                    created_at: d.created_at,
                    revoked_at: d.revoked_at,
                })
            })
            .collect::<Result<Vec<_>, TransportError>>()?,
        revocations: doc
            .revocations
            .into_iter()
            .map(|r| DirectoryRevocation {
                device_id: r.device_id,
                revoked_at: r.revoked_at,
            })
            .collect(),
    })
}

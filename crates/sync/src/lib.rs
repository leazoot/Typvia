// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! End-to-end encrypted synchronization client for Typvia.
//!
//! Device identity is per-device Ed25519 + X25519 key pairs whose private
//! halves live only in the platform [`typvia_crypto::SecureStore`]; the
//! fingerprint format, trust-root statement and certificate chain build on
//! it, as do pairing, provisioning, recovery and key rotation. The change-set
//! layer seals entity documents into wire records and opens them through a
//! verify pipeline with per-class rejection reasons and replay checks.
//! [`SyncTransport`] abstracts the server (HTTP and WebDAV), and
//! [`SyncEngine`] drives the outbox/push/pull loop over it.

mod base32;
mod cert;
mod directory;
mod engine;
mod error;
mod http;
mod identity;
mod keyring;
mod local;
mod merge;
mod pairing;
mod payload;
mod provision;
mod record;
mod recovery;
mod replay;
mod rotation;
mod transport;
mod webdav;
mod webdav_engine;
mod webdav_store;

pub use cert::{CertificateSubject, DeviceCertificate, RootStatement, verify_certificate_chain};
pub use directory::{
    TrustState, cert_chain_from_json, cert_chain_to_json, root_statement_from_json,
    root_statement_to_json,
};
pub use engine::{
    AdoptedAccount, NewAccount, OutboxSealer, SyncDb, SyncEngine, SyncError, SyncFailure,
    SyncReport, enqueue_change,
};
pub use error::{CertificateError, EnsureLocalDeviceError, IdentityError, RecordError};
pub use http::{HttpTransport, PROTOCOL_VERSION};
pub use identity::{DEVICE_ED25519_ENTRY, DEVICE_X25519_ENTRY, DeviceIdentity, Fingerprint};
pub use keyring::{
    K_SYNC_ENTRY_PREFIX, KeyringError, SyncKeys, install_key, provision_initial_key,
};
pub use local::{LocalDevice, LocalDeviceConfig, ensure_local_device};
pub use pairing::{
    KeyBundle, KeyGeneration, PAIRING_CODE_PREFIX, PairingCode, PairingError, SealedMessage,
    compute_sas, open_key_update, open_pair_bundle, seal_key_update, seal_pair_bundle,
};
pub use payload::{PayloadError, snippet_tag_entity_id};
pub use provision::{ClaimedPairing, PairingHandle};
pub use record::{
    MAX_RECORD_ENVELOPE_LEN, OpenedRecord, RecordMeta, SUPPORTED_PAYLOAD_VERSION, SyncKeyring,
    TrustContext, WireRecord, seal_record, seal_tombstone, verify_and_open,
};
pub use recovery::{
    RecoveryCode, RecoveryError, RecoveryRequest, build_recovery_blob, open_recovery_blob,
    rootproof_signing_key,
};
pub use replay::{AppliedHead, ServerSeqCursor, check_not_replayed};
pub use rotation::AccountDevice;
pub use transport::{
    ConflictHead, DeviceDirectory, DirectoryDevice, DirectoryRevocation, HandshakeInfo, KeyUpdate,
    PairClaim, PairingSession, PullPage, PulledRecord, PushOutcome, PushResultItem, ReRootOutcome,
    SessionToken, SyncTransport, TransportError,
};
pub use webdav::{FetchedFile, PutOutcome, WebdavClient, WebdavCredentials};
pub use webdav_store::{FetchedDirectory, WebdavAccount, WebdavStore};

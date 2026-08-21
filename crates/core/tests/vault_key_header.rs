// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! End-to-end key-header round-trip:
//! derive the KEK from the master password, wrap the MK and a domain key,
//! persist the header, load it back, and unlock. The red line: the correct
//! password unlocks; a wrong password fails.
//!
//! Uses cheap Argon2 parameters so the test stays fast — the round-trip
//! semantics do not depend on cost, and the real v1 cost is exercised in the
//! crypto crate. What matters here is that the persisted parameters and wrapped
//! bytes survive storage and drive a correct derive/unwrap.

#![allow(clippy::unwrap_used)]

use typvia_core::db::{migrate_to_latest, open_in_memory};
use typvia_core::model::{DomainKey, KeyDomain, VaultKeyHeader};
use typvia_core::repo::VaultKeyRepo;
use typvia_crypto::{CryptoError, SymmetricKey};
use typvia_crypto::{
    KdfParams, SALT_LEN, aad_domain_key, aad_master_key, derive_kek, unwrap_key, wrap_key,
};

const NOW: i64 = 1_700_000_000_000;

fn cheap_params() -> KdfParams {
    KdfParams {
        version: 1,
        m_cost_kib: 8,
        t_cost: 1,
        p_cost: 1,
        salt: [0x5Au8; SALT_LEN],
    }
}

#[test]
fn correct_password_unlocks_and_wrong_password_fails() {
    let mut conn = open_in_memory().unwrap();
    migrate_to_latest(&mut conn).unwrap();
    let repo = VaultKeyRepo::new(&conn);

    let password = b"correct horse battery staple";
    let params = cheap_params();

    // Initialize the vault: KEK from the password wraps a fresh MK; the MK
    // wraps a fresh vault-domain key.
    let kek = derive_kek(password, &params).unwrap();
    let mk = SymmetricKey::generate();
    let wrapped_mk = wrap_key(&kek, 1, &aad_master_key(), &mk).unwrap();

    let vault_key = SymmetricKey::generate();
    let wrapped_vault = wrap_key(
        &mk,
        1,
        &aad_domain_key(KeyDomain::Vault.as_str()),
        &vault_key,
    )
    .unwrap();

    repo.put_header(&VaultKeyHeader {
        id: "vault-header".to_string(),
        kdf: params,
        wrapped_mk,
        created_at: NOW,
        updated_at: NOW,
    })
    .unwrap();
    repo.put_domain_key(&DomainKey {
        domain: KeyDomain::Vault,
        key_id: 1,
        wrapped_key: wrapped_vault,
        created_at: NOW,
    })
    .unwrap();

    // Reload everything from storage — nothing is carried over in memory.
    let header = repo.load_header().unwrap().unwrap();
    let domain = repo
        .load_latest_domain_key(KeyDomain::Vault)
        .unwrap()
        .unwrap();

    // Correct password: re-derive the KEK from the *stored* parameters and
    // unwrap the MK, then the domain key.
    let kek_ok = derive_kek(password, &header.kdf).unwrap();
    let mk_again = unwrap_key(&kek_ok, &aad_master_key(), &header.wrapped_mk).unwrap();
    assert_eq!(mk_again.expose(), mk.expose(), "MK must survive round-trip");

    let vault_again = unwrap_key(
        &mk_again,
        &aad_domain_key(KeyDomain::Vault.as_str()),
        &domain.wrapped_key,
    )
    .unwrap();
    assert_eq!(
        vault_again.expose(),
        vault_key.expose(),
        "vault domain key must survive round-trip"
    );

    // Wrong password: a different KEK cannot unwrap the MK, and the error does
    // not say why.
    let kek_bad = derive_kek(b"wrong password", &header.kdf).unwrap();
    let err = unwrap_key(&kek_bad, &aad_master_key(), &header.wrapped_mk).unwrap_err();
    assert_eq!(err, CryptoError::DecryptionFailed);
}

#[test]
fn stored_kdf_parameters_survive_the_round_trip() {
    let mut conn = open_in_memory().unwrap();
    migrate_to_latest(&mut conn).unwrap();
    let repo = VaultKeyRepo::new(&conn);

    let params = cheap_params();
    let kek = derive_kek(b"pw", &params).unwrap();
    let mk = SymmetricKey::generate();
    let wrapped_mk = wrap_key(&kek, 1, &aad_master_key(), &mk).unwrap();

    repo.put_header(&VaultKeyHeader {
        id: "h".to_string(),
        kdf: params.clone(),
        wrapped_mk,
        created_at: NOW,
        updated_at: NOW,
    })
    .unwrap();

    let loaded = repo.load_header().unwrap().unwrap();
    assert_eq!(loaded.kdf, params, "KDF parameters must persist verbatim");
}

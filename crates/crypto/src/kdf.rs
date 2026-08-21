// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Argon2id master-password derivation.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::OsRng;
use chacha20poly1305::aead::rand_core::RngCore;

use crate::error::CryptoError;
use crate::keys::{KEY_LEN, SymmetricKey};

/// Salt length for password derivation.
pub const SALT_LEN: usize = 16;

const V1_M_COST_KIB: u32 = 64 * 1024;
const V1_T_COST: u32 = 3;
const V1_P_COST: u32 = 1;

/// Versioned Argon2id parameters, persisted alongside the key header so
/// old headers keep verifying after future tuning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KdfParams {
    pub version: u8,
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
    pub salt: [u8; SALT_LEN],
}

impl KdfParams {
    /// Current baseline (v1): 64 MiB, 3 iterations, parallelism 1, with a
    /// fresh random salt.
    pub fn v1() -> Self {
        let mut salt = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        Self::v1_with_salt(salt)
    }

    /// v1 parameters with a salt read back from a stored key header.
    pub fn v1_with_salt(salt: [u8; SALT_LEN]) -> Self {
        Self {
            version: 1,
            m_cost_kib: V1_M_COST_KIB,
            t_cost: V1_T_COST,
            p_cost: V1_P_COST,
            salt,
        }
    }
}

/// Derives the KEK from the master password. The caller owns the password
/// buffer and must zeroize it after use; the returned key zeroizes itself.
pub fn derive_kek(password: &[u8], params: &KdfParams) -> Result<SymmetricKey, CryptoError> {
    let argon_params = Params::new(
        params.m_cost_kib,
        params.t_cost,
        params.p_cost,
        Some(KEY_LEN),
    )
    .map_err(|_| CryptoError::KdfFailed)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut out = [0u8; KEY_LEN];
    argon
        .hash_password_into(password, &params.salt, &mut out)
        .map_err(|_| CryptoError::KdfFailed)?;
    let key = SymmetricKey::from_bytes(out);
    out.fill(0);
    Ok(key)
}

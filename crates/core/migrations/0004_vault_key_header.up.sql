-- Vault key-header persistence.
-- Stores only the KDF salt/parameters and wrapped (encrypted) key material.
-- No plaintext key ever lives here: the master password, KEK, and MK plaintext
-- never touch the database.

-- The master-key header. Singleton in practice (one MK per device vault); the
-- repository enforces the single row by replacing the whole table on write.
CREATE TABLE key_header (
    id TEXT PRIMARY KEY,
    -- Argon2id parameters, versioned so old headers keep verifying.
    kdf_version INTEGER NOT NULL CHECK (kdf_version >= 0),
    kdf_m_cost_kib INTEGER NOT NULL CHECK (kdf_m_cost_kib >= 0),
    kdf_t_cost INTEGER NOT NULL CHECK (kdf_t_cost >= 0),
    kdf_p_cost INTEGER NOT NULL CHECK (kdf_p_cost >= 0),
    kdf_salt BLOB NOT NULL CHECK (length(kdf_salt) = 16),
    -- MK wrapped under the KEK (envelope; AAD "typvia.mk.v1").
    wrapped_mk BLOB NOT NULL CHECK (length(wrapped_mk) > 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Per-domain keys (sync, vault), wrapped under the MK. Versioned by key_id so a
-- single domain can rotate independently; the newest generation is the
-- highest key_id for a domain.
CREATE TABLE domain_key (
    domain TEXT NOT NULL,
    key_id INTEGER NOT NULL CHECK (key_id >= 0),
    -- Domain key wrapped under the MK (envelope; AAD "typvia.domain.<domain>.v1").
    wrapped_key BLOB NOT NULL CHECK (length(wrapped_key) > 0),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (domain, key_id)
) WITHOUT ROWID;

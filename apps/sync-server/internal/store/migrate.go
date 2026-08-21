// Typvia sync server
// Copyright (C) 2026 Typvia contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at
// your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero
// General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

package store

import (
	"context"
	"database/sql"
	"fmt"
	"time"
)

// migration is one versioned schema step. Migrations are append-only: a
// released migration must never be edited, only followed by a new one.
type migration struct {
	version int64
	sql     string
}

// migrations holds the ordered schema history. Entities: account, device,
// record, pairing_session, key_update, recovery_blob, revocation. All content
// columns are opaque ciphertext or public material; nothing here is ever
// decrypted server-side.
var migrations = []migration{
	{
		version: 1,
		sql: `
CREATE TABLE account (
    id             TEXT PRIMARY KEY,
    root_statement BLOB NOT NULL,
    created_at     INTEGER NOT NULL
);

CREATE TABLE device (
    id          TEXT PRIMARY KEY,
    account_id  TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    platform    TEXT NOT NULL,
    ed25519_pub BLOB NOT NULL,
    x25519_pub  BLOB NOT NULL,
    cert_chain  BLOB NOT NULL,
    created_at  INTEGER NOT NULL,
    revoked_at  INTEGER
);
CREATE INDEX idx_device_account ON device(account_id);

CREATE TABLE record (
    id          TEXT PRIMARY KEY,
    account_id  TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    version     INTEGER NOT NULL,
    ciphertext  BLOB,
    deleted_at  INTEGER,
    updated_at  INTEGER NOT NULL,
    device_id   TEXT NOT NULL REFERENCES device(id),
    key_id      INTEGER NOT NULL,
    signature   BLOB NOT NULL,
    server_seq  INTEGER NOT NULL,
    UNIQUE (account_id, entity_type, entity_id, version),
    UNIQUE (account_id, server_seq),
    CHECK ((deleted_at IS NULL AND ciphertext IS NOT NULL)
        OR (deleted_at IS NOT NULL AND ciphertext IS NULL))
);

CREATE TABLE pairing_session (
    id         TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    offer      BLOB,
    claimed_at INTEGER
);

CREATE TABLE key_update (
    seq              INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id       TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    target_device_id TEXT NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    payload          BLOB NOT NULL,
    created_at       INTEGER NOT NULL
);
CREATE INDEX idx_key_update_target ON key_update(account_id, target_device_id, seq);

CREATE TABLE recovery_blob (
    account_id TEXT PRIMARY KEY REFERENCES account(id) ON DELETE CASCADE,
    blob       BLOB NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE revocation (
    account_id TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    device_id  TEXT NOT NULL REFERENCES device(id),
    statement  BLOB NOT NULL,
    revoked_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (account_id, device_id)
);
`,
	},
	{
		// Recovery re-root: the server must verify an
		// MK-possession proof without ever holding MK, so the client registers
		// the derived rootproof public key alongside the recovery blob. NULL
		// means no proof key is registered yet (re-root refused until a PUT
		// /v1/recovery supplies one).
		version: 2,
		sql:     `ALTER TABLE recovery_blob ADD COLUMN rootproof_pub BLOB`,
	},
}

// Migrate applies all pending migrations. Each migration runs inside its own
// transaction: a failure rolls back completely and leaves the previous schema
// version intact. Re-running is a no-op for already applied versions.
func (s *Store) Migrate(ctx context.Context) error {
	if _, err := s.db.ExecContext(ctx, `
CREATE TABLE IF NOT EXISTS schema_migrations (
    version    INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
)`); err != nil {
		return fmt.Errorf("create schema_migrations: %w", err)
	}

	for _, m := range migrations {
		applied, err := s.migrationApplied(ctx, m.version)
		if err != nil {
			return err
		}
		if applied {
			continue
		}
		err = s.inTx(ctx, func(tx *sql.Tx) error {
			if _, err := tx.ExecContext(ctx, m.sql); err != nil {
				return fmt.Errorf("apply migration %d: %w", m.version, err)
			}
			if _, err := tx.ExecContext(ctx,
				"INSERT INTO schema_migrations (version, applied_at) VALUES (?, ?)",
				m.version, time.Now().UnixMilli(),
			); err != nil {
				return fmt.Errorf("register migration %d: %w", m.version, err)
			}
			return nil
		})
		if err != nil {
			return err
		}
	}
	return nil
}

// SchemaVersion returns the highest applied migration version (0 if none).
func (s *Store) SchemaVersion(ctx context.Context) (int64, error) {
	var v sql.NullInt64
	err := s.db.QueryRowContext(ctx, "SELECT MAX(version) FROM schema_migrations").Scan(&v)
	if err != nil {
		return 0, fmt.Errorf("read schema version: %w", err)
	}
	return v.Int64, nil
}

func (s *Store) migrationApplied(ctx context.Context, version int64) (bool, error) {
	var n int
	err := s.db.QueryRowContext(ctx,
		"SELECT COUNT(*) FROM schema_migrations WHERE version = ?", version,
	).Scan(&n)
	if err != nil {
		return false, fmt.Errorf("check migration %d: %w", version, err)
	}
	return n > 0, nil
}

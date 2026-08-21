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
	"errors"
	"fmt"
	"strings"
)

// Record is one accepted sync record plus the server-assigned pull cursor
// server_seq. Ciphertext and signature are
// opaque bytes: the server never decrypts and only optionally verifies
// signatures as an admission filter (client verification is authoritative).
type Record struct {
	// ID is the client-generated UUID and idempotent dedup key.
	ID         string
	AccountID  string
	EntityType string
	EntityID   string
	Version    int64
	Ciphertext []byte
	DeletedAt  *int64
	UpdatedAt  int64
	DeviceID   string
	KeyID      int64
	Signature  []byte
	// ServerSeq is assigned by AppendRecord; zero on input.
	ServerSeq int64
}

// isTombstone reports the tombstone form: deleted_at set and no ciphertext
// (an invariant enforced here and by a CHECK constraint).
func (r Record) isTombstone() bool {
	return r.DeletedAt != nil
}

// AppendRecord accepts a record, assigning the next per-account server_seq
// (monotonic). Idempotent on record id: re-submitting an already stored
// id returns its existing server_seq with inserted=false. Returns
// ErrConflict when the (account, entity_type, entity_id, version) slot is
// already taken by a different record, ErrInvalid on invariant violations.
func (s *Store) AppendRecord(ctx context.Context, r Record) (serverSeq int64, inserted bool, err error) {
	if err := validateRecord(r); err != nil {
		return 0, false, err
	}
	err = s.inTx(ctx, func(tx *sql.Tx) error {
		var existingSeq int64
		scanErr := tx.QueryRowContext(ctx,
			"SELECT server_seq FROM record WHERE id = ?", r.ID,
		).Scan(&existingSeq)
		if scanErr == nil {
			serverSeq = existingSeq
			inserted = false
			return nil
		}
		if !errors.Is(scanErr, sql.ErrNoRows) {
			return fmt.Errorf("check record id: %w", scanErr)
		}

		var next int64
		if err := tx.QueryRowContext(ctx,
			"SELECT COALESCE(MAX(server_seq), 0) + 1 FROM record WHERE account_id = ?", r.AccountID,
		).Scan(&next); err != nil {
			return fmt.Errorf("allocate server_seq: %w", err)
		}

		// Tombstones store NULL ciphertext (not an empty blob) so the CHECK
		// constraint expresses the invariant exactly.
		var ciphertext any
		if !r.isTombstone() {
			ciphertext = r.Ciphertext
		}
		if _, err := tx.ExecContext(ctx, `
INSERT INTO record (id, account_id, entity_type, entity_id, version, ciphertext,
                    deleted_at, updated_at, device_id, key_id, signature, server_seq)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			r.ID, r.AccountID, r.EntityType, r.EntityID, r.Version, ciphertext,
			nullableInt(r.DeletedAt), r.UpdatedAt, r.DeviceID, r.KeyID, r.Signature, next,
		); err != nil {
			return mapConstraintErr(err)
		}
		serverSeq = next
		inserted = true
		return nil
	})
	if err != nil {
		return 0, false, err
	}
	return serverSeq, inserted, nil
}

// HeadVersion returns the current head version of an entity and whether any
// version exists (base_version check for push).
func (s *Store) HeadVersion(ctx context.Context, accountID, entityType, entityID string) (int64, bool, error) {
	var head sql.NullInt64
	err := s.db.QueryRowContext(ctx, `
SELECT MAX(version) FROM record
WHERE account_id = ? AND entity_type = ? AND entity_id = ?`,
		accountID, entityType, entityID,
	).Scan(&head)
	if err != nil {
		return 0, false, fmt.Errorf("head version: %w", err)
	}
	return head.Int64, head.Valid, nil
}

// ListRecordsSince returns up to limit records with server_seq > since in
// ascending server_seq order (incremental pull).
func (s *Store) ListRecordsSince(ctx context.Context, accountID string, since int64, limit int) ([]Record, error) {
	if limit <= 0 {
		return nil, fmt.Errorf("%w: limit must be positive", ErrInvalid)
	}
	rows, err := s.db.QueryContext(ctx, `
SELECT id, account_id, entity_type, entity_id, version, ciphertext,
       deleted_at, updated_at, device_id, key_id, signature, server_seq
FROM record
WHERE account_id = ? AND server_seq > ?
ORDER BY server_seq
LIMIT ?`, accountID, since, limit)
	if err != nil {
		return nil, fmt.Errorf("list records: %w", err)
	}
	defer rows.Close()

	var out []Record
	for rows.Next() {
		var r Record
		var deletedAt sql.NullInt64
		if err := rows.Scan(&r.ID, &r.AccountID, &r.EntityType, &r.EntityID, &r.Version, &r.Ciphertext,
			&deletedAt, &r.UpdatedAt, &r.DeviceID, &r.KeyID, &r.Signature, &r.ServerSeq); err != nil {
			return nil, fmt.Errorf("list records: %w", err)
		}
		r.DeletedAt = scanNullableInt(deletedAt)
		out = append(out, r)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("list records: %w", err)
	}
	return out, nil
}

// validateRecord enforces storage invariants before SQL: required fields,
// positive version, and the tombstone invariant (tombstone has no ciphertext,
// non-tombstone must carry ciphertext).
func validateRecord(r Record) error {
	if strings.TrimSpace(r.ID) == "" || strings.TrimSpace(r.AccountID) == "" ||
		strings.TrimSpace(r.EntityType) == "" || strings.TrimSpace(r.EntityID) == "" ||
		strings.TrimSpace(r.DeviceID) == "" {
		return fmt.Errorf("%w: record requires id, account, entity identity and device", ErrInvalid)
	}
	if r.Version <= 0 {
		return fmt.Errorf("%w: record version must be positive", ErrInvalid)
	}
	if len(r.Signature) == 0 {
		return fmt.Errorf("%w: record requires a signature", ErrInvalid)
	}
	if r.isTombstone() && len(r.Ciphertext) != 0 {
		return fmt.Errorf("%w: tombstone must not carry ciphertext", ErrInvalid)
	}
	if !r.isTombstone() && len(r.Ciphertext) == 0 {
		return fmt.Errorf("%w: non-tombstone must carry ciphertext", ErrInvalid)
	}
	return nil
}

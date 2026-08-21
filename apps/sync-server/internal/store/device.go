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

// Device is a registered device: public keys and certificate chain only.
// Private keys never reach the server.
type Device struct {
	ID         string
	AccountID  string
	Name       string
	Platform   string
	Ed25519Pub []byte
	X25519Pub  []byte
	// CertChain is the opaque certificate chain (new device back to the
	// trust root); clients are the verification authority.
	CertChain []byte
	CreatedAt int64
	// RevokedAt is nil for active devices (a device is revoked if and only if
	// RevokedAt is set).
	RevokedAt *int64
}

// Revocation is a device revocation statement signed by a trusted device;
// the server stores it and mirrors revoked_at onto the device row, but
// clients remain the verification authority.
type Revocation struct {
	AccountID string
	DeviceID  string
	Statement []byte
	RevokedAt int64
	CreatedAt int64
}

// validateDevice enforces the required-field invariant shared by
// CreateDevice and ReRoot.
func validateDevice(d Device) error {
	if strings.TrimSpace(d.ID) == "" || strings.TrimSpace(d.AccountID) == "" ||
		strings.TrimSpace(d.Name) == "" || strings.TrimSpace(d.Platform) == "" ||
		len(d.Ed25519Pub) == 0 || len(d.X25519Pub) == 0 || len(d.CertChain) == 0 {
		return fmt.Errorf("%w: device requires id, account, name, platform, public keys and cert chain", ErrInvalid)
	}
	return nil
}

// CreateDevice registers a device. Returns ErrConflict on duplicate id,
// ErrInvalid on missing required fields or unknown account.
func (s *Store) CreateDevice(ctx context.Context, d Device) error {
	if err := validateDevice(d); err != nil {
		return err
	}
	_, err := s.db.ExecContext(ctx, `
INSERT INTO device (id, account_id, name, platform, ed25519_pub, x25519_pub, cert_chain, created_at, revoked_at)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		d.ID, d.AccountID, d.Name, d.Platform, d.Ed25519Pub, d.X25519Pub, d.CertChain, d.CreatedAt, nullableInt(d.RevokedAt),
	)
	if err != nil {
		return mapConstraintErr(err)
	}
	return nil
}

// GetDevice fetches a device by id. Returns ErrNotFound if absent.
func (s *Store) GetDevice(ctx context.Context, id string) (Device, error) {
	row := s.db.QueryRowContext(ctx, `
SELECT id, account_id, name, platform, ed25519_pub, x25519_pub, cert_chain, created_at, revoked_at
FROM device WHERE id = ?`, id)
	d, err := scanDevice(row)
	if errors.Is(err, sql.ErrNoRows) {
		return Device{}, fmt.Errorf("%w: device", ErrNotFound)
	}
	if err != nil {
		return Device{}, fmt.Errorf("get device: %w", err)
	}
	return d, nil
}

// ListDevices returns all devices of an account (device directory endpoint).
func (s *Store) ListDevices(ctx context.Context, accountID string) ([]Device, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT id, account_id, name, platform, ed25519_pub, x25519_pub, cert_chain, created_at, revoked_at
FROM device WHERE account_id = ? ORDER BY created_at, id`, accountID)
	if err != nil {
		return nil, fmt.Errorf("list devices: %w", err)
	}
	defer rows.Close()

	var out []Device
	for rows.Next() {
		d, err := scanDevice(rows)
		if err != nil {
			return nil, fmt.Errorf("list devices: %w", err)
		}
		out = append(out, d)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("list devices: %w", err)
	}
	return out, nil
}

// AddRevocation stores a revocation statement and marks the device revoked in
// the same transaction (no partial state on failure). Returns ErrNotFound if
// the device is absent, ErrConflict if it is already revoked.
func (s *Store) AddRevocation(ctx context.Context, r Revocation) error {
	if len(r.Statement) == 0 {
		return fmt.Errorf("%w: revocation statement must not be empty", ErrInvalid)
	}
	return s.inTx(ctx, func(tx *sql.Tx) error {
		res, err := tx.ExecContext(ctx,
			"UPDATE device SET revoked_at = ? WHERE id = ? AND account_id = ? AND revoked_at IS NULL",
			r.RevokedAt, r.DeviceID, r.AccountID,
		)
		if err != nil {
			return fmt.Errorf("mark device revoked: %w", err)
		}
		n, err := res.RowsAffected()
		if err != nil {
			return fmt.Errorf("mark device revoked: %w", err)
		}
		if n == 0 {
			if _, err := s.deviceExistsTx(ctx, tx, r.AccountID, r.DeviceID); err != nil {
				return err
			}
			return fmt.Errorf("%w: device already revoked", ErrConflict)
		}
		if _, err := tx.ExecContext(ctx, `
INSERT INTO revocation (account_id, device_id, statement, revoked_at, created_at)
VALUES (?, ?, ?, ?, ?)`,
			r.AccountID, r.DeviceID, r.Statement, r.RevokedAt, r.CreatedAt,
		); err != nil {
			return mapConstraintErr(err)
		}
		return nil
	})
}

// ListRevocations returns all revocation statements of an account so clients
// can verify them against the trust chain.
func (s *Store) ListRevocations(ctx context.Context, accountID string) ([]Revocation, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT account_id, device_id, statement, revoked_at, created_at
FROM revocation WHERE account_id = ? ORDER BY created_at, device_id`, accountID)
	if err != nil {
		return nil, fmt.Errorf("list revocations: %w", err)
	}
	defer rows.Close()

	var out []Revocation
	for rows.Next() {
		var r Revocation
		if err := rows.Scan(&r.AccountID, &r.DeviceID, &r.Statement, &r.RevokedAt, &r.CreatedAt); err != nil {
			return nil, fmt.Errorf("list revocations: %w", err)
		}
		out = append(out, r)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("list revocations: %w", err)
	}
	return out, nil
}

func (s *Store) deviceExistsTx(ctx context.Context, tx *sql.Tx, accountID, deviceID string) (bool, error) {
	var n int
	err := tx.QueryRowContext(ctx,
		"SELECT COUNT(*) FROM device WHERE id = ? AND account_id = ?", deviceID, accountID,
	).Scan(&n)
	if err != nil {
		return false, fmt.Errorf("check device: %w", err)
	}
	if n == 0 {
		return false, fmt.Errorf("%w: device", ErrNotFound)
	}
	return true, nil
}

type rowScanner interface {
	Scan(dest ...any) error
}

func scanDevice(r rowScanner) (Device, error) {
	var d Device
	var revokedAt sql.NullInt64
	err := r.Scan(&d.ID, &d.AccountID, &d.Name, &d.Platform, &d.Ed25519Pub, &d.X25519Pub, &d.CertChain, &d.CreatedAt, &revokedAt)
	if err != nil {
		return Device{}, err
	}
	d.RevokedAt = scanNullableInt(revokedAt)
	return d, nil
}

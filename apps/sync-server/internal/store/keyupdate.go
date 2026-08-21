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
	"fmt"
	"strings"
)

// KeyUpdate is a sealed device-to-device key message: domain key rotation or
// late vault authorization. Payload is opaque sealed bytes; the server can
// never open it.
type KeyUpdate struct {
	// Seq is the server-assigned pull cursor for ?since= (assigned on insert).
	Seq            int64
	AccountID      string
	TargetDeviceID string
	Payload        []byte
	CreatedAt      int64
}

// AddKeyUpdate stores a sealed key update and returns its assigned seq.
// Returns ErrInvalid on missing fields or unknown account/target device.
func (s *Store) AddKeyUpdate(ctx context.Context, k KeyUpdate) (int64, error) {
	if strings.TrimSpace(k.AccountID) == "" || strings.TrimSpace(k.TargetDeviceID) == "" || len(k.Payload) == 0 {
		return 0, fmt.Errorf("%w: key update requires account, target device and payload", ErrInvalid)
	}
	res, err := s.db.ExecContext(ctx, `
INSERT INTO key_update (account_id, target_device_id, payload, created_at)
VALUES (?, ?, ?, ?)`,
		k.AccountID, k.TargetDeviceID, k.Payload, k.CreatedAt,
	)
	if err != nil {
		return 0, mapConstraintErr(err)
	}
	seq, err := res.LastInsertId()
	if err != nil {
		return 0, fmt.Errorf("add key update: %w", err)
	}
	return seq, nil
}

// ListKeyUpdatesSince returns key updates addressed to a device with
// seq > since, in ascending seq order.
func (s *Store) ListKeyUpdatesSince(ctx context.Context, accountID, targetDeviceID string, since int64) ([]KeyUpdate, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT seq, account_id, target_device_id, payload, created_at
FROM key_update
WHERE account_id = ? AND target_device_id = ? AND seq > ?
ORDER BY seq`, accountID, targetDeviceID, since)
	if err != nil {
		return nil, fmt.Errorf("list key updates: %w", err)
	}
	defer rows.Close()

	var out []KeyUpdate
	for rows.Next() {
		var k KeyUpdate
		if err := rows.Scan(&k.Seq, &k.AccountID, &k.TargetDeviceID, &k.Payload, &k.CreatedAt); err != nil {
			return nil, fmt.Errorf("list key updates: %w", err)
		}
		out = append(out, k)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("list key updates: %w", err)
	}
	return out, nil
}

// DeleteKeyUpdatesThrough removes the messages a device has acknowledged by
// pulling past them (`since` doubles as the acknowledgment).
// Each row is addressed to exactly one device, so dropping the acknowledged
// prefix cannot take a message away from another reader. Returns rows removed.
func (s *Store) DeleteKeyUpdatesThrough(ctx context.Context, accountID, targetDeviceID string, through int64) (int64, error) {
	if through <= 0 {
		return 0, nil
	}
	res, err := s.db.ExecContext(ctx, `
DELETE FROM key_update
WHERE account_id = ? AND target_device_id = ? AND seq <= ?`,
		accountID, targetDeviceID, through,
	)
	if err != nil {
		return 0, fmt.Errorf("delete key updates: %w", err)
	}
	removed, err := res.RowsAffected()
	if err != nil {
		return 0, fmt.Errorf("delete key updates: %w", err)
	}
	return removed, nil
}

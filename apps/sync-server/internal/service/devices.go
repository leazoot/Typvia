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

package service

import (
	"context"
	"crypto/ed25519"
	"encoding/json"
	"errors"
	"fmt"

	"typvia.dev/sync-server/internal/store"
	"typvia.dev/sync-server/internal/wire"
)

// DeviceOut is one device directory row: public keys and certificate chain
// only, clients verify the chain themselves.
type DeviceOut struct {
	ID         string          `json:"id"`
	Name       string          `json:"name"`
	Platform   string          `json:"platform"`
	Ed25519Pub []byte          `json:"ed25519_pub"`
	X25519Pub  []byte          `json:"x25519_pub"`
	CertChain  json.RawMessage `json:"cert_chain"`
	CreatedAt  int64           `json:"created_at"`
	RevokedAt  *int64          `json:"revoked_at"`
}

// RevocationOut is one stored revocation statement for client-side
// verification.
type RevocationOut struct {
	DeviceID  string          `json:"device_id"`
	Statement json.RawMessage `json:"statement"`
	RevokedAt int64           `json:"revoked_at"`
	CreatedAt int64           `json:"created_at"`
}

// DirectoryOut is the device directory response: trust root, devices with
// certificate chains, and revocation statements.
type DirectoryOut struct {
	RootStatement json.RawMessage `json:"root_statement"`
	Devices       []DeviceOut     `json:"devices"`
	Revocations   []RevocationOut `json:"revocations"`
}

// revocationDoc is the stored JSON form of a revocation statement, carrying
// everything clients need to re-verify it.
type revocationDoc struct {
	IssuerDeviceID  string `json:"issuer_device_id"`
	RevokedDeviceID string `json:"revoked_device_id"`
	RevokedAt       int64  `json:"revoked_at"`
	Signature       []byte `json:"signature"`
}

// DeviceDirectory returns the full device directory of the session account.
func (s *Service) DeviceDirectory(ctx context.Context, sess Session) (DirectoryOut, error) {
	account, err := s.store.GetAccount(ctx, sess.AccountID)
	if errors.Is(err, store.ErrNotFound) {
		return DirectoryOut{}, NotFoundErr("account is not registered")
	}
	if err != nil {
		return DirectoryOut{}, systemErr(fmt.Errorf("load account: %w", err))
	}
	devices, err := s.store.ListDevices(ctx, sess.AccountID)
	if err != nil {
		return DirectoryOut{}, systemErr(fmt.Errorf("list devices: %w", err))
	}
	revocations, err := s.store.ListRevocations(ctx, sess.AccountID)
	if err != nil {
		return DirectoryOut{}, systemErr(fmt.Errorf("list revocations: %w", err))
	}

	out := DirectoryOut{
		RootStatement: json.RawMessage(account.RootStatement),
		Devices:       make([]DeviceOut, len(devices)),
		Revocations:   make([]RevocationOut, len(revocations)),
	}
	for i, d := range devices {
		out.Devices[i] = DeviceOut{
			ID:         d.ID,
			Name:       d.Name,
			Platform:   d.Platform,
			Ed25519Pub: d.Ed25519Pub,
			X25519Pub:  d.X25519Pub,
			CertChain:  json.RawMessage(d.CertChain),
			CreatedAt:  d.CreatedAt,
			RevokedAt:  d.RevokedAt,
		}
	}
	for i, r := range revocations {
		out.Revocations[i] = RevocationOut{
			DeviceID:  r.DeviceID,
			Statement: json.RawMessage(r.Statement),
			RevokedAt: r.RevokedAt,
			CreatedAt: r.CreatedAt,
		}
	}
	return out, nil
}

// RevokeDevice stores a revocation statement after verifying it against the
// issuing (authenticated) device's key (a signature over
// "typvia.revoke.v1" || revoked_device_id || revoked_at). The revocation
// statement and the device revoked_at mark land in one transaction; the
// revoked device's sessions and pending challenges are invalidated.
func (s *Service) RevokeDevice(ctx context.Context, sess Session, targetDeviceID string, revokedAt int64, sig []byte) error {
	if err := validateIDField("device id", targetDeviceID, maxIDLen); err != nil {
		return err
	}
	if revokedAt <= 0 {
		return MalformedErr("revoked_at must be a positive millisecond timestamp")
	}
	if len(sig) != ed25519.SignatureSize {
		return MalformedErr("signature must be a 64-byte Ed25519 signature")
	}
	issuer, err := s.activeDevice(ctx, sess.DeviceID, sess.AccountID)
	if err != nil {
		return err
	}
	target, err := s.store.GetDevice(ctx, targetDeviceID)
	if errors.Is(err, store.ErrNotFound) {
		return NotFoundErr("device is not registered")
	}
	if err != nil {
		return systemErr(fmt.Errorf("load device: %w", err))
	}
	if target.AccountID != sess.AccountID {
		return NotFoundErr("device is not registered")
	}
	if err := wire.Verify(issuer.Ed25519Pub, wire.RevokeSignedBytes(targetDeviceID, revokedAt), sig); err != nil {
		return MalformedErr("revocation signature verification failed")
	}

	statement, err := json.Marshal(revocationDoc{
		IssuerDeviceID:  issuer.ID,
		RevokedDeviceID: targetDeviceID,
		RevokedAt:       revokedAt,
		Signature:       sig,
	})
	if err != nil {
		return systemErr(fmt.Errorf("encode revocation statement: %w", err))
	}
	err = s.store.AddRevocation(ctx, store.Revocation{
		AccountID: sess.AccountID,
		DeviceID:  targetDeviceID,
		Statement: statement,
		RevokedAt: revokedAt,
		CreatedAt: s.nowMillis(),
	})
	if errors.Is(err, store.ErrConflict) {
		return businessErr(CodeDeviceRevoked, "device is already revoked")
	}
	if errors.Is(err, store.ErrNotFound) {
		return NotFoundErr("device is not registered")
	}
	if err != nil {
		return systemErr(fmt.Errorf("store revocation: %w", err))
	}
	s.sessions.dropDevice(targetDeviceID)
	return nil
}

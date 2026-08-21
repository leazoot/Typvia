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
	"errors"
	"fmt"

	"typvia.dev/sync-server/internal/store"
	"typvia.dev/sync-server/internal/wire"
)

// maxRecoveryBlobSize bounds the recovery blob (a wrapped key bundle plus
// KDF header).
const maxRecoveryBlobSize = 256 * 1024

// PutRecoveryBlob stores or replaces the account recovery blob (replacing
// invalidates the previous recovery code). rootproofPub is the
// public key of the MK-derived rootproof keypair — the server verifies
// re-root proofs against it without ever holding MK.
func (s *Service) PutRecoveryBlob(ctx context.Context, sess Session, blob, rootproofPub []byte) error {
	if len(blob) == 0 {
		return MalformedErr("blob is required")
	}
	if len(blob) > maxRecoveryBlobSize {
		return PayloadTooLargeErr(fmt.Sprintf("blob exceeds %d bytes", maxRecoveryBlobSize))
	}
	if len(rootproofPub) != ed25519.PublicKeySize {
		return MalformedErr("rootproof_pub must be a 32-byte Ed25519 public key")
	}
	err := s.store.UpsertRecoveryBlob(ctx, store.RecoveryBlob{
		AccountID:    sess.AccountID,
		Blob:         blob,
		RootproofPub: rootproofPub,
		UpdatedAt:    s.nowMillis(),
	})
	if err != nil {
		return systemErr(fmt.Errorf("store recovery blob: %w", err))
	}
	return nil
}

// GetRecoveryBlob returns the account recovery blob (recovery flow;
// anonymous, rate limiting happens at the handler). The blob is useless
// without the recovery code.
func (s *Service) GetRecoveryBlob(ctx context.Context, accountID string) ([]byte, error) {
	if err := validateIDField("account_id", accountID, maxIDLen); err != nil {
		return nil, err
	}
	blob, err := s.store.GetRecoveryBlob(ctx, accountID)
	if errors.Is(err, store.ErrNotFound) {
		return nil, NotFoundErr("no recovery blob is stored for this account")
	}
	if err != nil {
		return nil, systemErr(fmt.Errorf("load recovery blob: %w", err))
	}
	return blob.Blob, nil
}

// IssueRootChallenge starts the re-root proof exchange: a one-time
// challenge the recovering device signs with the MK-derived rootproof key.
// Refused when no rootproof key is registered.
func (s *Service) IssueRootChallenge(ctx context.Context, accountID string) (ChallengeOut, error) {
	if err := validateIDField("account_id", accountID, maxIDLen); err != nil {
		return ChallengeOut{}, err
	}
	if _, err := s.rootproofPub(ctx, accountID); err != nil {
		return ChallengeOut{}, err
	}
	value, err := newSecret(32)
	if err != nil {
		return ChallengeOut{}, systemErr(fmt.Errorf("generate challenge: %w", err))
	}
	s.sessions.putRootChallenge(accountID, value, s.now().Add(challengeTTL))
	return ChallengeOut{Challenge: value, ExpiresIn: int(challengeTTL.Seconds())}, nil
}

// ReRoot executes the recovery root replacement: the MK-possession proof
// signature is verified against the registered rootproof key over the
// one-time root challenge, the new self-signed root statement is admitted,
// and in one transaction the trust root is replaced, every previous
// device is marked revoked, and the new root device is registered. All
// sessions of the account are invalidated.
func (s *Service) ReRoot(ctx context.Context, accountID string, proofSig []byte, newRoot RootDeviceInput) error {
	if err := validateIDField("account_id", accountID, maxIDLen); err != nil {
		return err
	}
	if len(proofSig) != ed25519.SignatureSize {
		return MalformedErr("proof_sig must be a 64-byte Ed25519 signature")
	}
	pub, err := s.rootproofPub(ctx, accountID)
	if err != nil {
		return err
	}
	// The challenge burns on the attempt (one-time), before any verification.
	challengeValue, ok := s.sessions.takeRootChallenge(accountID, s.now())
	if !ok {
		return businessErr(CodeAuthChallengeExpired, "no valid root challenge for this account; request a new one")
	}
	if err := wire.Verify(pub, wire.RootProofSignedBytes(challengeValue, accountID), proofSig); err != nil {
		return MalformedErr("root proof signature verification failed")
	}
	if err := validateRootDevice(newRoot); err != nil {
		return err
	}

	// Collect current device ids before the swap so their pending challenges
	// can be dropped alongside the account sessions.
	devices, err := s.store.ListDevices(ctx, accountID)
	if err != nil {
		return systemErr(fmt.Errorf("list devices: %w", err))
	}
	deviceIDs := make([]string, len(devices))
	for i, d := range devices {
		deviceIDs[i] = d.ID
	}

	now := s.nowMillis()
	device, statement, err := rootDeviceRow(accountID, newRoot, now)
	if err != nil {
		return err
	}
	err = s.store.ReRoot(ctx, accountID, statement, device, now)
	if errors.Is(err, store.ErrNotFound) {
		return NotFoundErr("account is not registered")
	}
	if errors.Is(err, store.ErrConflict) {
		return MalformedErr("device is already registered")
	}
	if err != nil {
		return systemErr(fmt.Errorf("replace trust root: %w", err))
	}
	s.sessions.dropAccount(accountID, deviceIDs)
	return nil
}

// rootproofPub loads the registered rootproof public key of an account. The
// answer is uniform for "unknown account" and "no blob/proof key stored":
// re-root is simply not available.
func (s *Service) rootproofPub(ctx context.Context, accountID string) ([]byte, error) {
	blob, err := s.store.GetRecoveryBlob(ctx, accountID)
	if errors.Is(err, store.ErrNotFound) {
		return nil, NotFoundErr("re-root is not available for this account")
	}
	if err != nil {
		return nil, systemErr(fmt.Errorf("load recovery blob: %w", err))
	}
	if len(blob.RootproofPub) != ed25519.PublicKeySize {
		return nil, NotFoundErr("re-root is not available for this account")
	}
	return blob.RootproofPub, nil
}

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
	"errors"
	"testing"
)

// pairingFixture prepares an account and one pairing session.
func pairingFixture(t *testing.T) *Store {
	t.Helper()
	s := newTestStore(t)
	seedAccount(t, s, "acc-1")
	err := s.CreatePairingSession(context.Background(), PairingSession{
		ID: "sess-1", AccountID: "acc-1", CreatedAt: 1000, ExpiresAt: 601000,
	})
	if err != nil {
		t.Fatalf("seed pairing session: %v", err)
	}
	return s
}

func TestPairingSessionRoundTrip(t *testing.T) {
	t.Parallel()
	s := pairingFixture(t)
	got, err := s.GetPairingSession(context.Background(), "sess-1")
	if err != nil {
		t.Fatalf("get session: %v", err)
	}
	if got.AccountID != "acc-1" || got.ExpiresAt != 601000 || got.Offer != nil || got.ClaimedAt != nil {
		t.Fatalf("unexpected session: %+v", got)
	}
}

func TestCreatePairingSessionRejectsNonPositiveTTL(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	seedAccount(t, s, "acc-1")
	err := s.CreatePairingSession(context.Background(), PairingSession{
		ID: "sess-1", AccountID: "acc-1", CreatedAt: 1000, ExpiresAt: 1000,
	})
	if !errors.Is(err, ErrInvalid) {
		t.Fatalf("want ErrInvalid, got %v", err)
	}
}

// pairedDevice builds a valid device row for the offer's registration side
// effect (FAKE key material).
func pairedDevice(accountID, deviceID string) Device {
	return Device{
		ID:         deviceID,
		AccountID:  accountID,
		Name:       "FAKE_PAIRED_NAME",
		Platform:   "ios",
		Ed25519Pub: []byte("FAKE_ED25519_PUB_32_BYTES_______"),
		X25519Pub:  []byte("FAKE_X25519_PUB_32_BYTES________"),
		CertChain:  []byte(`[{"fake":"cert"}]`),
		CreatedAt:  1500,
	}
}

func TestSetPairingOfferWithDeviceIsSingleShotAndRegisters(t *testing.T) {
	t.Parallel()
	s := pairingFixture(t)
	ctx := context.Background()

	err := s.SetPairingOfferWithDevice(ctx, "sess-1", []byte("FAKE_SEALED_OFFER"), pairedDevice("acc-1", "dev-new"))
	if err != nil {
		t.Fatalf("set offer: %v", err)
	}
	got, err := s.GetPairingSession(ctx, "sess-1")
	if err != nil {
		t.Fatalf("get session: %v", err)
	}
	if string(got.Offer) != "FAKE_SEALED_OFFER" {
		t.Fatalf("offer must round-trip opaquely, got %q", got.Offer)
	}
	// The registration side effect landed in the same transaction.
	d, err := s.GetDevice(ctx, "dev-new")
	if err != nil {
		t.Fatalf("get registered device: %v", err)
	}
	if d.AccountID != "acc-1" || string(d.CertChain) != `[{"fake":"cert"}]` {
		t.Fatalf("unexpected registered device: %+v", d)
	}

	err = s.SetPairingOfferWithDevice(ctx, "sess-1", []byte("second"), pairedDevice("acc-1", "dev-other"))
	if !errors.Is(err, ErrConflict) {
		t.Fatalf("second offer: want ErrConflict, got %v", err)
	}
	// The second device registration rolled back with the refused offer.
	if _, err := s.GetDevice(ctx, "dev-other"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("device of refused offer must not exist, got %v", err)
	}
}

func TestSetPairingOfferWithDeviceReturnsNotFoundForUnknownSession(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	seedAccount(t, s, "acc-1")
	err := s.SetPairingOfferWithDevice(context.Background(), "missing", []byte("offer"), pairedDevice("acc-1", "dev-new"))
	if !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
	// Nothing registered on the failed path.
	if _, err := s.GetDevice(context.Background(), "dev-new"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("device must not exist, got %v", err)
	}
}

func TestSetPairingOfferWithDuplicateDeviceRollsBackTheOffer(t *testing.T) {
	t.Parallel()
	s := pairingFixture(t)
	ctx := context.Background()
	seedDevice(t, s, "acc-1", "dev-existing")

	err := s.SetPairingOfferWithDevice(ctx, "sess-1", []byte("offer"), pairedDevice("acc-1", "dev-existing"))
	if !errors.Is(err, ErrConflict) {
		t.Fatalf("want ErrConflict, got %v", err)
	}
	// The offer write rolled back with the failed registration: the
	// session stays usable for a corrected offer.
	got, err := s.GetPairingSession(ctx, "sess-1")
	if err != nil {
		t.Fatalf("get session: %v", err)
	}
	if got.Offer != nil {
		t.Fatalf("offer must have rolled back, got %q", got.Offer)
	}
}

func TestClaimPairingSessionIsSingleUse(t *testing.T) {
	t.Parallel()
	s := pairingFixture(t)
	ctx := context.Background()

	if err := s.ClaimPairingSession(ctx, "sess-1", 2000); err != nil {
		t.Fatalf("claim: %v", err)
	}
	got, err := s.GetPairingSession(ctx, "sess-1")
	if err != nil {
		t.Fatalf("get session: %v", err)
	}
	if got.ClaimedAt == nil || *got.ClaimedAt != 2000 {
		t.Fatalf("session not marked claimed: %+v", got)
	}

	err = s.ClaimPairingSession(ctx, "sess-1", 3000)
	if !errors.Is(err, ErrConflict) {
		t.Fatalf("second claim: want ErrConflict, got %v", err)
	}
}

func TestDeleteExpiredPairingSessionsRemovesOnlyExpired(t *testing.T) {
	t.Parallel()
	s := pairingFixture(t) // sess-1 expires at 601000
	ctx := context.Background()
	err := s.CreatePairingSession(ctx, PairingSession{
		ID: "sess-2", AccountID: "acc-1", CreatedAt: 1000, ExpiresAt: 900000,
	})
	if err != nil {
		t.Fatalf("seed second session: %v", err)
	}

	n, err := s.DeleteExpiredPairingSessions(ctx, 700000)
	if err != nil {
		t.Fatalf("delete expired: %v", err)
	}
	if n != 1 {
		t.Fatalf("want 1 deleted, got %d", n)
	}
	if _, err := s.GetPairingSession(ctx, "sess-1"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("expired session must be gone, got %v", err)
	}
	if _, err := s.GetPairingSession(ctx, "sess-2"); err != nil {
		t.Fatalf("live session must survive, got %v", err)
	}
}

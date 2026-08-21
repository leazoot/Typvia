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
	"strings"
	"testing"
)

// testRecord builds a valid non-tombstone record with fake ciphertext.
func testRecord(id, accountID, deviceID, entityID string, version int64) Record {
	return Record{
		ID:         id,
		AccountID:  accountID,
		EntityType: "snippet",
		EntityID:   entityID,
		Version:    version,
		Ciphertext: []byte("FAKE_CIPHERTEXT_" + id),
		UpdatedAt:  1000,
		DeviceID:   deviceID,
		KeyID:      1,
		Signature:  []byte("FAKE_SIGNATURE_" + id),
	}
}

// recordFixture prepares an account and device for record tests.
func recordFixture(t *testing.T) *Store {
	t.Helper()
	s := newTestStore(t)
	seedAccount(t, s, "acc-1")
	seedDevice(t, s, "acc-1", "dev-1")
	return s
}

func TestAppendRecordAssignsMonotonicServerSeqPerAccount(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	ctx := context.Background()
	seedAccount(t, s, "acc-2")
	seedDevice(t, s, "acc-2", "dev-2")

	for i, rec := range []Record{
		testRecord("r1", "acc-1", "dev-1", "e1", 1),
		testRecord("r2", "acc-1", "dev-1", "e2", 1),
		testRecord("r3", "acc-1", "dev-1", "e1", 2),
	} {
		seq, inserted, err := s.AppendRecord(ctx, rec)
		if err != nil {
			t.Fatalf("append %s: %v", rec.ID, err)
		}
		if !inserted || seq != int64(i+1) {
			t.Fatalf("append %s: want seq %d inserted, got seq %d inserted=%v", rec.ID, i+1, seq, inserted)
		}
	}

	// A second account starts its own sequence at 1.
	seq, inserted, err := s.AppendRecord(ctx, testRecord("r4", "acc-2", "dev-2", "e1", 1))
	if err != nil || !inserted || seq != 1 {
		t.Fatalf("append to second account: want seq 1 inserted, got seq %d inserted=%v err=%v", seq, inserted, err)
	}
}

func TestAppendRecordIsIdempotentOnRecordID(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	ctx := context.Background()

	rec := testRecord("r1", "acc-1", "dev-1", "e1", 1)
	first, _, err := s.AppendRecord(ctx, rec)
	if err != nil {
		t.Fatalf("first append: %v", err)
	}
	again, inserted, err := s.AppendRecord(ctx, rec)
	if err != nil {
		t.Fatalf("retry append: %v", err)
	}
	if inserted || again != first {
		t.Fatalf("retry must return existing seq %d without inserting, got seq %d inserted=%v", first, again, inserted)
	}
}

func TestAppendRecordRejectsOccupiedVersionSlot(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	ctx := context.Background()

	if _, _, err := s.AppendRecord(ctx, testRecord("r1", "acc-1", "dev-1", "e1", 1)); err != nil {
		t.Fatalf("first append: %v", err)
	}
	// Different record id, same (account, entity_type, entity_id, version).
	_, _, err := s.AppendRecord(ctx, testRecord("r2", "acc-1", "dev-1", "e1", 1))
	if !errors.Is(err, ErrConflict) {
		t.Fatalf("want ErrConflict, got %v", err)
	}
}

func TestAppendRecordAcceptsTombstoneWithoutCiphertext(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	ctx := context.Background()

	deletedAt := int64(2000)
	rec := testRecord("r1", "acc-1", "dev-1", "e1", 1)
	rec.Ciphertext = nil
	rec.DeletedAt = &deletedAt

	if _, _, err := s.AppendRecord(ctx, rec); err != nil {
		t.Fatalf("append tombstone: %v", err)
	}
	got, err := s.ListRecordsSince(ctx, "acc-1", 0, 10)
	if err != nil {
		t.Fatalf("list: %v", err)
	}
	if len(got) != 1 || got[0].DeletedAt == nil || *got[0].DeletedAt != 2000 || got[0].Ciphertext != nil {
		t.Fatalf("unexpected tombstone row: %+v", got[0])
	}
}

func TestAppendRecordRejectsTombstoneWithCiphertext(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	deletedAt := int64(2000)
	rec := testRecord("r1", "acc-1", "dev-1", "e1", 1)
	rec.DeletedAt = &deletedAt // keeps fake ciphertext: invalid combination

	_, _, err := s.AppendRecord(context.Background(), rec)
	if !errors.Is(err, ErrInvalid) {
		t.Fatalf("want ErrInvalid, got %v", err)
	}
}

func TestAppendRecordRejectsNonTombstoneWithoutCiphertext(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	rec := testRecord("r1", "acc-1", "dev-1", "e1", 1)
	rec.Ciphertext = nil

	_, _, err := s.AppendRecord(context.Background(), rec)
	if !errors.Is(err, ErrInvalid) {
		t.Fatalf("want ErrInvalid, got %v", err)
	}
}

func TestTombstoneInvariantIsEnforcedByCheckConstraint(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)

	// Bypass Go-side validation to prove the schema CHECK is a real backstop
	// (tombstone has no ciphertext, non-tombstone must carry ciphertext).
	_, err := s.db.Exec(`
INSERT INTO record (id, account_id, entity_type, entity_id, version, ciphertext,
                    deleted_at, updated_at, device_id, key_id, signature, server_seq)
VALUES ('bad1', 'acc-1', 'snippet', 'e1', 1, X'AB', 2000, 1000, 'dev-1', 1, X'CD', 1)`)
	if err == nil || !strings.Contains(err.Error(), "CHECK") {
		t.Fatalf("tombstone with ciphertext must violate CHECK, got %v", err)
	}

	_, err = s.db.Exec(`
INSERT INTO record (id, account_id, entity_type, entity_id, version, ciphertext,
                    deleted_at, updated_at, device_id, key_id, signature, server_seq)
VALUES ('bad2', 'acc-1', 'snippet', 'e1', 1, NULL, NULL, 1000, 'dev-1', 1, X'CD', 1)`)
	if err == nil || !strings.Contains(err.Error(), "CHECK") {
		t.Fatalf("non-tombstone without ciphertext must violate CHECK, got %v", err)
	}
}

func TestHeadVersionTracksHighestVersion(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	ctx := context.Background()

	_, exists, err := s.HeadVersion(ctx, "acc-1", "snippet", "e1")
	if err != nil {
		t.Fatalf("head version: %v", err)
	}
	if exists {
		t.Fatal("head must not exist before any record")
	}

	for v := int64(1); v <= 3; v++ {
		rec := testRecord("r"+string(rune('0'+v)), "acc-1", "dev-1", "e1", v)
		if _, _, err := s.AppendRecord(ctx, rec); err != nil {
			t.Fatalf("append v%d: %v", v, err)
		}
	}
	head, exists, err := s.HeadVersion(ctx, "acc-1", "snippet", "e1")
	if err != nil {
		t.Fatalf("head version: %v", err)
	}
	if !exists || head != 3 {
		t.Fatalf("want head 3, got %d (exists=%v)", head, exists)
	}
}

func TestListRecordsSinceReturnsIncrementalOrderedPage(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	ctx := context.Background()

	for i := 1; i <= 4; i++ {
		rec := testRecord("r"+string(rune('0'+i)), "acc-1", "dev-1", "e"+string(rune('0'+i)), 1)
		if _, _, err := s.AppendRecord(ctx, rec); err != nil {
			t.Fatalf("append %d: %v", i, err)
		}
	}

	got, err := s.ListRecordsSince(ctx, "acc-1", 1, 2)
	if err != nil {
		t.Fatalf("list: %v", err)
	}
	if len(got) != 2 || got[0].ServerSeq != 2 || got[1].ServerSeq != 3 {
		t.Fatalf("want seqs [2 3], got %+v", got)
	}
	if string(got[0].Ciphertext) != "FAKE_CIPHERTEXT_r2" {
		t.Fatalf("ciphertext must round-trip opaquely, got %q", got[0].Ciphertext)
	}
}

func TestListRecordsSinceRejectsNonPositiveLimit(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	_, err := s.ListRecordsSince(context.Background(), "acc-1", 0, 0)
	if !errors.Is(err, ErrInvalid) {
		t.Fatalf("want ErrInvalid, got %v", err)
	}
}

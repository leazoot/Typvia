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

func TestAddKeyUpdateAssignsAscendingSeqAndListsSince(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	ctx := context.Background()
	seedAccount(t, s, "acc-1")
	seedDevice(t, s, "acc-1", "dev-1")
	seedDevice(t, s, "acc-1", "dev-2")

	var seqs []int64
	for _, target := range []string{"dev-1", "dev-1", "dev-2"} {
		seq, err := s.AddKeyUpdate(ctx, KeyUpdate{
			AccountID: "acc-1", TargetDeviceID: target,
			Payload: []byte("FAKE_SEALED_KEY_UPDATE_" + target), CreatedAt: 1000,
		})
		if err != nil {
			t.Fatalf("add key update for %s: %v", target, err)
		}
		seqs = append(seqs, seq)
	}
	if !(seqs[0] < seqs[1] && seqs[1] < seqs[2]) {
		t.Fatalf("seqs must ascend, got %v", seqs)
	}

	// dev-1 sees only its own updates after the first seq.
	got, err := s.ListKeyUpdatesSince(ctx, "acc-1", "dev-1", seqs[0])
	if err != nil {
		t.Fatalf("list key updates: %v", err)
	}
	if len(got) != 1 || got[0].Seq != seqs[1] || string(got[0].Payload) != "FAKE_SEALED_KEY_UPDATE_dev-1" {
		t.Fatalf("unexpected key updates: %+v", got)
	}
}

func TestAddKeyUpdateRejectsUnknownTargetDevice(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	seedAccount(t, s, "acc-1")
	_, err := s.AddKeyUpdate(context.Background(), KeyUpdate{
		AccountID: "acc-1", TargetDeviceID: "missing", Payload: []byte("p"), CreatedAt: 1,
	})
	if !errors.Is(err, ErrInvalid) {
		t.Fatalf("want ErrInvalid (foreign key), got %v", err)
	}
}

func TestUpsertRecoveryBlobReplacesPreviousBlob(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	ctx := context.Background()
	seedAccount(t, s, "acc-1")

	first := RecoveryBlob{AccountID: "acc-1", Blob: []byte("FAKE_RECOVERY_BLOB_V1"), UpdatedAt: 1000}
	if err := s.UpsertRecoveryBlob(ctx, first); err != nil {
		t.Fatalf("first upsert: %v", err)
	}
	second := RecoveryBlob{AccountID: "acc-1", Blob: []byte("FAKE_RECOVERY_BLOB_V2"), UpdatedAt: 2000}
	if err := s.UpsertRecoveryBlob(ctx, second); err != nil {
		t.Fatalf("second upsert: %v", err)
	}

	got, err := s.GetRecoveryBlob(ctx, "acc-1")
	if err != nil {
		t.Fatalf("get blob: %v", err)
	}
	if string(got.Blob) != "FAKE_RECOVERY_BLOB_V2" || got.UpdatedAt != 2000 {
		t.Fatalf("blob not replaced: %+v", got)
	}
}

func TestGetRecoveryBlobReturnsNotFoundWhenAbsent(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	seedAccount(t, s, "acc-1")
	_, err := s.GetRecoveryBlob(context.Background(), "acc-1")
	if !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
}

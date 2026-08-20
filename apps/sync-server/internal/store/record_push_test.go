package store

import (
	"context"
	"errors"
	"testing"
)

func TestPushRecordsAppliesBatchWithMonotonicSeq(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	ctx := context.Background()

	results, err := s.PushRecords(ctx, "acc-1", []Record{
		testRecord("p1", "acc-1", "dev-1", "e1", 1),
		testRecord("p2", "acc-1", "dev-1", "e2", 1),
		testRecord("p3", "acc-1", "dev-1", "e1", 2),
	})
	if err != nil {
		t.Fatalf("push batch: %v", err)
	}
	for i, want := range []int64{1, 2, 3} {
		if results[i].ServerSeq != want || results[i].Duplicate {
			t.Fatalf("result %d: want seq %d non-duplicate, got %+v", i, want, results[i])
		}
	}
}

func TestPushRecordsIsIdempotentOnRecordID(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	ctx := context.Background()

	first, err := s.PushRecords(ctx, "acc-1", []Record{testRecord("p1", "acc-1", "dev-1", "e1", 1)})
	if err != nil {
		t.Fatalf("first push: %v", err)
	}
	again, err := s.PushRecords(ctx, "acc-1", []Record{
		testRecord("p1", "acc-1", "dev-1", "e1", 1),
		testRecord("p2", "acc-1", "dev-1", "e1", 2),
	})
	if err != nil {
		t.Fatalf("second push: %v", err)
	}
	if !again[0].Duplicate || again[0].ServerSeq != first[0].ServerSeq {
		t.Fatalf("want duplicate with seq %d, got %+v", first[0].ServerSeq, again[0])
	}
	if again[1].Duplicate || again[1].ServerSeq != first[0].ServerSeq+1 {
		t.Fatalf("want new record at seq %d, got %+v", first[0].ServerSeq+1, again[1])
	}
}

func TestPushRecordsRejectsNonHeadVersionWithCurrentHeads(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	ctx := context.Background()

	if _, err := s.PushRecords(ctx, "acc-1", []Record{testRecord("p1", "acc-1", "dev-1", "e1", 1)}); err != nil {
		t.Fatalf("seed head: %v", err)
	}

	// Version 1 replays a taken slot; version 3 skips the head+1 rule.
	for _, bad := range []int64{1, 3} {
		_, err := s.PushRecords(ctx, "acc-1", []Record{testRecord("px", "acc-1", "dev-1", "e1", bad)})
		var conflict *VersionConflictError
		if !errors.As(err, &conflict) {
			t.Fatalf("version %d: want VersionConflictError, got %v", bad, err)
		}
		if len(conflict.Heads) != 1 || conflict.Heads[0].EntityID != "e1" || conflict.Heads[0].HeadVersion != 1 {
			t.Fatalf("version %d: want head e1@1, got %+v", bad, conflict.Heads)
		}
	}
}

func TestPushRecordsConflictAppliesNothing(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	ctx := context.Background()

	_, err := s.PushRecords(ctx, "acc-1", []Record{
		testRecord("p1", "acc-1", "dev-1", "e1", 1),
		testRecord("p2", "acc-1", "dev-1", "e2", 5), // conflicts (head 0)
	})
	var conflict *VersionConflictError
	if !errors.As(err, &conflict) {
		t.Fatalf("want VersionConflictError, got %v", err)
	}
	if len(conflict.Heads) != 1 || conflict.Heads[0].EntityID != "e2" || conflict.Heads[0].HeadVersion != 0 {
		t.Fatalf("want head e2@0, got %+v", conflict.Heads)
	}
	// The valid p1 must not survive the rolled-back batch.
	if _, exists, err := s.HeadVersion(ctx, "acc-1", "snippet", "e1"); err != nil || exists {
		t.Fatalf("want no head for e1 after rollback, exists=%v err=%v", exists, err)
	}
}

func TestPushRecordsCountsEarlierBatchRecordsTowardHead(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)
	ctx := context.Background()

	// Offline backlog: v1 then v2 of the same entity in one batch is legal.
	results, err := s.PushRecords(ctx, "acc-1", []Record{
		testRecord("p1", "acc-1", "dev-1", "e1", 1),
		testRecord("p2", "acc-1", "dev-1", "e1", 2),
	})
	if err != nil {
		t.Fatalf("push backlog batch: %v", err)
	}
	if results[1].ServerSeq != 2 {
		t.Fatalf("want second record at seq 2, got %+v", results[1])
	}
}

func TestPushRecordsRejectsAccountMismatch(t *testing.T) {
	t.Parallel()
	s := recordFixture(t)

	_, err := s.PushRecords(context.Background(), "acc-other", []Record{
		testRecord("p1", "acc-1", "dev-1", "e1", 1),
	})
	if !errors.Is(err, ErrInvalid) {
		t.Fatalf("want ErrInvalid on account mismatch, got %v", err)
	}
}

func TestReRootReplacesRootAndRevokesPreviousDevices(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	ctx := context.Background()
	seedAccount(t, s, "acc-1")
	seedDevice(t, s, "acc-1", "dev-old")

	newRoot := Device{
		ID:         "dev-new-root",
		AccountID:  "acc-1",
		Name:       "FAKE_RECOVERED",
		Platform:   "macos",
		Ed25519Pub: []byte("FAKE_ED25519_PUB"),
		X25519Pub:  []byte("FAKE_X25519_PUB"),
		CertChain:  []byte("[]"),
		CreatedAt:  2000,
	}
	if err := s.ReRoot(ctx, "acc-1", []byte("FAKE_NEW_ROOT_STATEMENT"), newRoot, 2000); err != nil {
		t.Fatalf("re-root: %v", err)
	}

	acc, err := s.GetAccount(ctx, "acc-1")
	if err != nil || string(acc.RootStatement) != "FAKE_NEW_ROOT_STATEMENT" {
		t.Fatalf("want replaced root statement, got %q err=%v", acc.RootStatement, err)
	}
	old, err := s.GetDevice(ctx, "dev-old")
	if err != nil || old.RevokedAt == nil || *old.RevokedAt != 2000 {
		t.Fatalf("want old device revoked at 2000, got %+v err=%v", old.RevokedAt, err)
	}
	fresh, err := s.GetDevice(ctx, "dev-new-root")
	if err != nil || fresh.RevokedAt != nil {
		t.Fatalf("want new root device active, got %+v err=%v", fresh, err)
	}
}

func TestReRootRejectsUnknownAccountAndTakenDeviceID(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	ctx := context.Background()
	seedAccount(t, s, "acc-1")
	seedDevice(t, s, "acc-1", "dev-old")

	newRoot := Device{
		ID: "dev-old", AccountID: "acc-1", Name: "n", Platform: "macos",
		Ed25519Pub: []byte("k"), X25519Pub: []byte("k"), CertChain: []byte("[]"), CreatedAt: 1,
	}
	if err := s.ReRoot(ctx, "acc-1", []byte("FAKE_ROOT"), newRoot, 1); !errors.Is(err, ErrConflict) {
		t.Fatalf("want ErrConflict on taken device id, got %v", err)
	}
	// The failed re-root must not have revoked anything.
	old, err := s.GetDevice(ctx, "dev-old")
	if err != nil || old.RevokedAt != nil {
		t.Fatalf("want dev-old untouched after failed re-root, got %+v err=%v", old, err)
	}

	missing := newRoot
	missing.ID = "dev-new"
	missing.AccountID = "acc-missing"
	if err := s.ReRoot(ctx, "acc-missing", []byte("FAKE_ROOT"), missing, 1); !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound on unknown account, got %v", err)
	}
}

func TestRecoveryBlobStoresRootproofPub(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	ctx := context.Background()
	seedAccount(t, s, "acc-1")

	blob := RecoveryBlob{
		AccountID:    "acc-1",
		Blob:         []byte("FAKE_RECOVERY_BLOB"),
		RootproofPub: []byte("FAKE_ROOTPROOF_PUB"),
		UpdatedAt:    10,
	}
	if err := s.UpsertRecoveryBlob(ctx, blob); err != nil {
		t.Fatalf("upsert recovery blob: %v", err)
	}
	got, err := s.GetRecoveryBlob(ctx, "acc-1")
	if err != nil || string(got.RootproofPub) != "FAKE_ROOTPROOF_PUB" {
		t.Fatalf("want stored rootproof pub, got %+v err=%v", got, err)
	}
}

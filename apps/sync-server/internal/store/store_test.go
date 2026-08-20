package store

import (
	"context"
	"errors"
	"path/filepath"
	"testing"
)

// newTestStore opens a migrated store on a per-test temporary database.
func newTestStore(t *testing.T) *Store {
	t.Helper()
	s, err := Open(filepath.Join(t.TempDir(), "test.db"))
	if err != nil {
		t.Fatalf("open store: %v", err)
	}
	t.Cleanup(func() {
		if err := s.Close(); err != nil {
			t.Errorf("close store: %v", err)
		}
	})
	if err := s.Migrate(context.Background()); err != nil {
		t.Fatalf("migrate: %v", err)
	}
	return s
}

// seedAccount creates an account with a deterministic fake root statement.
func seedAccount(t *testing.T, s *Store, id string) {
	t.Helper()
	err := s.CreateAccount(context.Background(), Account{
		ID:            id,
		RootStatement: []byte("fake-root-statement-" + id),
		CreatedAt:     1000,
	})
	if err != nil {
		t.Fatalf("seed account %s: %v", id, err)
	}
}

// seedDevice registers a device with fake (non-secret) key material.
func seedDevice(t *testing.T, s *Store, accountID, deviceID string) {
	t.Helper()
	err := s.CreateDevice(context.Background(), Device{
		ID:         deviceID,
		AccountID:  accountID,
		Name:       "Test Device " + deviceID,
		Platform:   "macos",
		Ed25519Pub: []byte("FAKE_ED25519_PUB_" + deviceID),
		X25519Pub:  []byte("FAKE_X25519_PUB_" + deviceID),
		CertChain:  []byte("FAKE_CERT_CHAIN_" + deviceID),
		CreatedAt:  1000,
	})
	if err != nil {
		t.Fatalf("seed device %s: %v", deviceID, err)
	}
}

func TestPingSucceedsOnOpenDatabase(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	if err := s.Ping(context.Background()); err != nil {
		t.Fatalf("ping: %v", err)
	}
}

func TestAccountRoundTrip(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	ctx := context.Background()
	seedAccount(t, s, "acc-1")

	got, err := s.GetAccount(ctx, "acc-1")
	if err != nil {
		t.Fatalf("get account: %v", err)
	}
	if got.ID != "acc-1" || string(got.RootStatement) != "fake-root-statement-acc-1" || got.CreatedAt != 1000 {
		t.Fatalf("unexpected account: %+v", got)
	}
}

func TestGetAccountReturnsNotFoundForUnknownID(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	_, err := s.GetAccount(context.Background(), "missing")
	if !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
}

func TestCreateAccountRejectsDuplicateID(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	seedAccount(t, s, "acc-1")
	err := s.CreateAccount(context.Background(), Account{
		ID: "acc-1", RootStatement: []byte("other"), CreatedAt: 2000,
	})
	if !errors.Is(err, ErrConflict) {
		t.Fatalf("want ErrConflict, got %v", err)
	}
}

func TestCreateAccountRejectsEmptyRootStatement(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	err := s.CreateAccount(context.Background(), Account{ID: "acc-1", CreatedAt: 1000})
	if !errors.Is(err, ErrInvalid) {
		t.Fatalf("want ErrInvalid, got %v", err)
	}
}

func TestReplaceRootStatementUpdatesExistingAccount(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	ctx := context.Background()
	seedAccount(t, s, "acc-1")

	if err := s.ReplaceRootStatement(ctx, "acc-1", []byte("new-root")); err != nil {
		t.Fatalf("replace root: %v", err)
	}
	got, err := s.GetAccount(ctx, "acc-1")
	if err != nil {
		t.Fatalf("get account: %v", err)
	}
	if string(got.RootStatement) != "new-root" {
		t.Fatalf("root statement not replaced: %q", got.RootStatement)
	}
}

func TestReplaceRootStatementReturnsNotFoundForUnknownAccount(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	err := s.ReplaceRootStatement(context.Background(), "missing", []byte("new-root"))
	if !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
}

func TestCreateDeviceRejectsUnknownAccount(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	err := s.CreateDevice(context.Background(), Device{
		ID: "dev-1", AccountID: "missing", Name: "n", Platform: "macos",
		Ed25519Pub: []byte("pk"), X25519Pub: []byte("pk"), CertChain: []byte("cc"), CreatedAt: 1,
	})
	if !errors.Is(err, ErrInvalid) {
		t.Fatalf("want ErrInvalid (foreign key), got %v", err)
	}
}

func TestDeviceRoundTripAndListing(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	ctx := context.Background()
	seedAccount(t, s, "acc-1")
	seedDevice(t, s, "acc-1", "dev-1")
	seedDevice(t, s, "acc-1", "dev-2")

	got, err := s.GetDevice(ctx, "dev-1")
	if err != nil {
		t.Fatalf("get device: %v", err)
	}
	if got.AccountID != "acc-1" || got.Platform != "macos" || got.RevokedAt != nil {
		t.Fatalf("unexpected device: %+v", got)
	}

	devices, err := s.ListDevices(ctx, "acc-1")
	if err != nil {
		t.Fatalf("list devices: %v", err)
	}
	if len(devices) != 2 || devices[0].ID != "dev-1" || devices[1].ID != "dev-2" {
		t.Fatalf("unexpected device list: %+v", devices)
	}
}

func TestAddRevocationMarksDeviceAndStoresStatement(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	ctx := context.Background()
	seedAccount(t, s, "acc-1")
	seedDevice(t, s, "acc-1", "dev-1")

	rev := Revocation{
		AccountID: "acc-1", DeviceID: "dev-1",
		Statement: []byte("FAKE_SIGNED_REVOCATION"), RevokedAt: 5000, CreatedAt: 5000,
	}
	if err := s.AddRevocation(ctx, rev); err != nil {
		t.Fatalf("add revocation: %v", err)
	}

	dev, err := s.GetDevice(ctx, "dev-1")
	if err != nil {
		t.Fatalf("get device: %v", err)
	}
	if dev.RevokedAt == nil || *dev.RevokedAt != 5000 {
		t.Fatalf("device not marked revoked: %+v", dev)
	}

	revs, err := s.ListRevocations(ctx, "acc-1")
	if err != nil {
		t.Fatalf("list revocations: %v", err)
	}
	if len(revs) != 1 || revs[0].DeviceID != "dev-1" || string(revs[0].Statement) != "FAKE_SIGNED_REVOCATION" {
		t.Fatalf("unexpected revocations: %+v", revs)
	}
}

func TestAddRevocationRejectsSecondRevocation(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	ctx := context.Background()
	seedAccount(t, s, "acc-1")
	seedDevice(t, s, "acc-1", "dev-1")
	rev := Revocation{AccountID: "acc-1", DeviceID: "dev-1", Statement: []byte("st"), RevokedAt: 1, CreatedAt: 1}
	if err := s.AddRevocation(ctx, rev); err != nil {
		t.Fatalf("first revocation: %v", err)
	}
	err := s.AddRevocation(ctx, rev)
	if !errors.Is(err, ErrConflict) {
		t.Fatalf("want ErrConflict, got %v", err)
	}
}

func TestAddRevocationReturnsNotFoundForUnknownDevice(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	seedAccount(t, s, "acc-1")
	err := s.AddRevocation(context.Background(), Revocation{
		AccountID: "acc-1", DeviceID: "missing", Statement: []byte("st"), RevokedAt: 1, CreatedAt: 1,
	})
	if !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
}

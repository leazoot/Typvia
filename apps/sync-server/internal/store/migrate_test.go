package store

import (
	"context"
	"path/filepath"
	"testing"
)

func TestMigrateCreatesAllStorageEntities(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)

	// The full storage entity set.
	expected := []string{
		"account", "device", "record", "pairing_session",
		"key_update", "recovery_blob", "revocation",
	}
	for _, table := range expected {
		var n int
		err := s.db.QueryRow(
			"SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?", table,
		).Scan(&n)
		if err != nil {
			t.Fatalf("query sqlite_master: %v", err)
		}
		if n != 1 {
			t.Errorf("table %q missing after migration", table)
		}
	}
}

// latestVersion is the highest version in the append-only migration history.
func latestVersion() int64 {
	return migrations[len(migrations)-1].version
}

func TestMigrateRecordsSchemaVersion(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	v, err := s.SchemaVersion(context.Background())
	if err != nil {
		t.Fatalf("schema version: %v", err)
	}
	if v != latestVersion() {
		t.Fatalf("want schema version %d, got %d", latestVersion(), v)
	}
}

func TestMigrateIsIdempotent(t *testing.T) {
	t.Parallel()
	ctx := context.Background()
	s := newTestStore(t) // first migration ran in the helper

	if err := s.Migrate(ctx); err != nil {
		t.Fatalf("second migrate must be a no-op, got %v", err)
	}
	var applied int
	if err := s.db.QueryRow("SELECT COUNT(*) FROM schema_migrations").Scan(&applied); err != nil {
		t.Fatalf("count applied migrations: %v", err)
	}
	if applied != len(migrations) {
		t.Fatalf("each migration must be registered exactly once, got %d rows for %d migrations", applied, len(migrations))
	}
}

func TestMigrateSurvivesReopen(t *testing.T) {
	t.Parallel()
	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "reopen.db")

	s, err := Open(path)
	if err != nil {
		t.Fatalf("open: %v", err)
	}
	if err := s.Migrate(ctx); err != nil {
		t.Fatalf("migrate: %v", err)
	}
	if err := s.Close(); err != nil {
		t.Fatalf("close: %v", err)
	}

	s2, err := Open(path)
	if err != nil {
		t.Fatalf("reopen: %v", err)
	}
	defer func() {
		if err := s2.Close(); err != nil {
			t.Errorf("close reopened store: %v", err)
		}
	}()
	if err := s2.Migrate(ctx); err != nil {
		t.Fatalf("migrate on reopen must be a no-op, got %v", err)
	}
	v, err := s2.SchemaVersion(ctx)
	if err != nil {
		t.Fatalf("schema version: %v", err)
	}
	if v != latestVersion() {
		t.Fatalf("want schema version %d after reopen, got %d", latestVersion(), v)
	}
}

func TestForeignKeysAreEnforced(t *testing.T) {
	t.Parallel()
	s := newTestStore(t)
	var fk int
	if err := s.db.QueryRow("PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatalf("read foreign_keys pragma: %v", err)
	}
	if fk != 1 {
		t.Fatal("foreign_keys pragma must be ON")
	}
}

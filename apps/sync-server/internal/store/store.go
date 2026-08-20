// Package store is the persistence layer of the sync server and the only
// place SQL lives. It stores ciphertext as opaque bytes and metadata only:
// the server holds no keys and performs no decryption.
package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"

	"modernc.org/sqlite"
	sqlitelib "modernc.org/sqlite/lib"
)

// Sentinel errors returned by store operations. The service layer maps them
// to the protocol's stable error codes.
var (
	// ErrNotFound: the requested row does not exist.
	ErrNotFound = errors.New("store: not found")
	// ErrConflict: a uniqueness constraint was violated (e.g. an already
	// occupied (account_id, entity_type, entity_id, version) slot).
	ErrConflict = errors.New("store: conflict")
	// ErrInvalid: the input violates a storage invariant (e.g. the tombstone
	// invariant).
	ErrInvalid = errors.New("store: invalid input")
)

// Store wraps the SQLite database. All methods use parameterized SQL only.
type Store struct {
	db *sql.DB
}

// Open opens (creating if needed) the SQLite database at path and applies
// the connection pragmas the schema relies on (foreign keys, WAL).
func Open(path string) (*Store, error) {
	dsn := "file:" + path + "?_pragma=foreign_keys(1)&_pragma=journal_mode(WAL)&_pragma=busy_timeout(5000)"
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("open sqlite: %w", err)
	}
	// SQLite allows a single writer; serializing all access through one
	// connection avoids SQLITE_BUSY between pooled connections.
	db.SetMaxOpenConns(1)
	return &Store{db: db}, nil
}

// Close closes the underlying database.
func (s *Store) Close() error {
	return s.db.Close()
}

// Ping verifies the database is reachable. Used by the health endpoint.
func (s *Store) Ping(ctx context.Context) error {
	var one int
	if err := s.db.QueryRowContext(ctx, "SELECT 1").Scan(&one); err != nil {
		return fmt.Errorf("ping: %w", err)
	}
	return nil
}

// inTx runs fn inside a transaction, committing on nil and rolling back on
// error so no partial write survives a failure.
func (s *Store) inTx(ctx context.Context, fn func(tx *sql.Tx) error) error {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	if err := fn(tx); err != nil {
		_ = tx.Rollback()
		return err
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit tx: %w", err)
	}
	return nil
}

// mapConstraintErr converts SQLite constraint violations to store sentinel
// errors; other errors pass through unchanged.
func mapConstraintErr(err error) error {
	var se *sqlite.Error
	if errors.As(err, &se) {
		switch se.Code() {
		case sqlitelib.SQLITE_CONSTRAINT_UNIQUE, sqlitelib.SQLITE_CONSTRAINT_PRIMARYKEY:
			return fmt.Errorf("%w: %v", ErrConflict, err)
		case sqlitelib.SQLITE_CONSTRAINT_CHECK, sqlitelib.SQLITE_CONSTRAINT_FOREIGNKEY, sqlitelib.SQLITE_CONSTRAINT_NOTNULL:
			return fmt.Errorf("%w: %v", ErrInvalid, err)
		}
	}
	return err
}

// nullableInt converts an optional millisecond timestamp to its SQL form.
func nullableInt(v *int64) any {
	if v == nil {
		return nil
	}
	return *v
}

// scanNullableInt converts a scanned nullable integer back to *int64.
func scanNullableInt(v sql.NullInt64) *int64 {
	if !v.Valid {
		return nil
	}
	n := v.Int64
	return &n
}

package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
)

// Account is a sync account: one trust root, one device set. root_statement
// is the first device's self-signed root statement, stored as opaque signed
// bytes the server never interprets.
type Account struct {
	ID            string
	RootStatement []byte
	CreatedAt     int64
}

// CreateAccount inserts a new account. Returns ErrConflict if the id exists,
// ErrInvalid on blank id or empty root statement.
func (s *Store) CreateAccount(ctx context.Context, a Account) error {
	if strings.TrimSpace(a.ID) == "" || len(a.RootStatement) == 0 {
		return fmt.Errorf("%w: account requires id and root statement", ErrInvalid)
	}
	_, err := s.db.ExecContext(ctx,
		"INSERT INTO account (id, root_statement, created_at) VALUES (?, ?, ?)",
		a.ID, a.RootStatement, a.CreatedAt,
	)
	if err != nil {
		return mapConstraintErr(err)
	}
	return nil
}

// CreateAccountWithDevice registers a new account and its first (root)
// device in one transaction (account creation is the first device's
// registration) — no orphan account survives a failure.
func (s *Store) CreateAccountWithDevice(ctx context.Context, a Account, d Device) error {
	if strings.TrimSpace(a.ID) == "" || len(a.RootStatement) == 0 {
		return fmt.Errorf("%w: account requires id and root statement", ErrInvalid)
	}
	if err := validateDevice(d); err != nil {
		return err
	}
	if d.AccountID != a.ID {
		return fmt.Errorf("%w: device account mismatch", ErrInvalid)
	}
	return s.inTx(ctx, func(tx *sql.Tx) error {
		if _, err := tx.ExecContext(ctx,
			"INSERT INTO account (id, root_statement, created_at) VALUES (?, ?, ?)",
			a.ID, a.RootStatement, a.CreatedAt,
		); err != nil {
			return mapConstraintErr(err)
		}
		if _, err := tx.ExecContext(ctx, `
INSERT INTO device (id, account_id, name, platform, ed25519_pub, x25519_pub, cert_chain, created_at, revoked_at)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			d.ID, d.AccountID, d.Name, d.Platform, d.Ed25519Pub, d.X25519Pub, d.CertChain, d.CreatedAt, nullableInt(d.RevokedAt),
		); err != nil {
			return mapConstraintErr(err)
		}
		return nil
	})
}

// GetAccount fetches an account by id. Returns ErrNotFound if absent.
func (s *Store) GetAccount(ctx context.Context, id string) (Account, error) {
	var a Account
	err := s.db.QueryRowContext(ctx,
		"SELECT id, root_statement, created_at FROM account WHERE id = ?", id,
	).Scan(&a.ID, &a.RootStatement, &a.CreatedAt)
	if errors.Is(err, sql.ErrNoRows) {
		return Account{}, fmt.Errorf("%w: account", ErrNotFound)
	}
	if err != nil {
		return Account{}, fmt.Errorf("get account: %w", err)
	}
	return a, nil
}

// ReplaceRootStatement swaps the account trust root (recovery re-root).
// Returns ErrNotFound if the account is absent.
func (s *Store) ReplaceRootStatement(ctx context.Context, accountID string, rootStatement []byte) error {
	if len(rootStatement) == 0 {
		return fmt.Errorf("%w: root statement must not be empty", ErrInvalid)
	}
	res, err := s.db.ExecContext(ctx,
		"UPDATE account SET root_statement = ? WHERE id = ?", rootStatement, accountID,
	)
	if err != nil {
		return fmt.Errorf("replace root statement: %w", err)
	}
	n, err := res.RowsAffected()
	if err != nil {
		return fmt.Errorf("replace root statement: %w", err)
	}
	if n == 0 {
		return fmt.Errorf("%w: account", ErrNotFound)
	}
	return nil
}

// ReRoot performs the recovery root replacement in one transaction: the
// account root statement is replaced, every device that
// is not yet revoked is marked revoked at revokedAt (their certificates no
// longer chain to the new root), and the new root device is registered.
// Returns ErrNotFound if the account is absent, ErrConflict if the new
// device id is already taken.
func (s *Store) ReRoot(ctx context.Context, accountID string, rootStatement []byte, newRoot Device, revokedAt int64) error {
	if len(rootStatement) == 0 {
		return fmt.Errorf("%w: root statement must not be empty", ErrInvalid)
	}
	if newRoot.AccountID != accountID {
		return fmt.Errorf("%w: new root device account mismatch", ErrInvalid)
	}
	if err := validateDevice(newRoot); err != nil {
		return err
	}
	return s.inTx(ctx, func(tx *sql.Tx) error {
		res, err := tx.ExecContext(ctx,
			"UPDATE account SET root_statement = ? WHERE id = ?", rootStatement, accountID,
		)
		if err != nil {
			return fmt.Errorf("replace root statement: %w", err)
		}
		n, err := res.RowsAffected()
		if err != nil {
			return fmt.Errorf("replace root statement: %w", err)
		}
		if n == 0 {
			return fmt.Errorf("%w: account", ErrNotFound)
		}
		if _, err := tx.ExecContext(ctx,
			"UPDATE device SET revoked_at = ? WHERE account_id = ? AND revoked_at IS NULL",
			revokedAt, accountID,
		); err != nil {
			return fmt.Errorf("revoke previous devices: %w", err)
		}
		if _, err := tx.ExecContext(ctx, `
INSERT INTO device (id, account_id, name, platform, ed25519_pub, x25519_pub, cert_chain, created_at, revoked_at)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
			newRoot.ID, newRoot.AccountID, newRoot.Name, newRoot.Platform,
			newRoot.Ed25519Pub, newRoot.X25519Pub, newRoot.CertChain, newRoot.CreatedAt,
		); err != nil {
			return mapConstraintErr(err)
		}
		return nil
	})
}

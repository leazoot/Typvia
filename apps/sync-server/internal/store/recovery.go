package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
)

// RecoveryBlob is the recovery-code-wrapped key bundle: useless without the
// recovery code, one per account, replaced on regeneration. Blob is opaque
// bytes; RootproofPub is the Ed25519 public key of the MK-derived rootproof
// keypair used to authorize re-root — public material only, nil when not
// registered.
type RecoveryBlob struct {
	AccountID    string
	Blob         []byte
	RootproofPub []byte
	UpdatedAt    int64
}

// UpsertRecoveryBlob stores or replaces the account recovery blob (replacing
// invalidates the previous recovery code by definition).
func (s *Store) UpsertRecoveryBlob(ctx context.Context, r RecoveryBlob) error {
	if strings.TrimSpace(r.AccountID) == "" || len(r.Blob) == 0 {
		return fmt.Errorf("%w: recovery blob requires account and content", ErrInvalid)
	}
	var pub any
	if len(r.RootproofPub) > 0 {
		pub = r.RootproofPub
	}
	_, err := s.db.ExecContext(ctx, `
INSERT INTO recovery_blob (account_id, blob, rootproof_pub, updated_at) VALUES (?, ?, ?, ?)
ON CONFLICT(account_id) DO UPDATE SET blob = excluded.blob,
    rootproof_pub = excluded.rootproof_pub, updated_at = excluded.updated_at`,
		r.AccountID, r.Blob, pub, r.UpdatedAt,
	)
	if err != nil {
		return mapConstraintErr(err)
	}
	return nil
}

// GetRecoveryBlob fetches the account recovery blob. Returns ErrNotFound if
// none was stored.
func (s *Store) GetRecoveryBlob(ctx context.Context, accountID string) (RecoveryBlob, error) {
	var r RecoveryBlob
	err := s.db.QueryRowContext(ctx,
		"SELECT account_id, blob, rootproof_pub, updated_at FROM recovery_blob WHERE account_id = ?", accountID,
	).Scan(&r.AccountID, &r.Blob, &r.RootproofPub, &r.UpdatedAt)
	if errors.Is(err, sql.ErrNoRows) {
		return RecoveryBlob{}, fmt.Errorf("%w: recovery blob", ErrNotFound)
	}
	if err != nil {
		return RecoveryBlob{}, fmt.Errorf("get recovery blob: %w", err)
	}
	return r, nil
}

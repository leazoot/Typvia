package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
)

// PairingSession is a short-lived pairing relay slot (TTL 10 min). Offer
// holds the trusted device's relay payload (certificate
// chain + sealed key bundle) as opaque bytes: only ciphertext and public
// material transit the server.
type PairingSession struct {
	ID        string
	AccountID string
	CreatedAt int64
	ExpiresAt int64
	// Offer is nil until the trusted device posts its relay payload.
	Offer []byte
	// ClaimedAt is set when the new device takes the offer (claim is
	// single-use).
	ClaimedAt *int64
}

// CreatePairingSession inserts a new pairing session. Returns ErrConflict on
// duplicate id, ErrInvalid on blank ids or non-positive TTL bounds.
func (s *Store) CreatePairingSession(ctx context.Context, p PairingSession) error {
	if strings.TrimSpace(p.ID) == "" || strings.TrimSpace(p.AccountID) == "" {
		return fmt.Errorf("%w: pairing session requires id and account", ErrInvalid)
	}
	if p.ExpiresAt <= p.CreatedAt {
		return fmt.Errorf("%w: pairing session must expire after creation", ErrInvalid)
	}
	_, err := s.db.ExecContext(ctx, `
INSERT INTO pairing_session (id, account_id, created_at, expires_at, offer, claimed_at)
VALUES (?, ?, ?, ?, NULL, NULL)`,
		p.ID, p.AccountID, p.CreatedAt, p.ExpiresAt,
	)
	if err != nil {
		return mapConstraintErr(err)
	}
	return nil
}

// GetPairingSession fetches a session by id. Returns ErrNotFound if absent.
func (s *Store) GetPairingSession(ctx context.Context, id string) (PairingSession, error) {
	var p PairingSession
	var claimedAt sql.NullInt64
	err := s.db.QueryRowContext(ctx, `
SELECT id, account_id, created_at, expires_at, offer, claimed_at
FROM pairing_session WHERE id = ?`, id,
	).Scan(&p.ID, &p.AccountID, &p.CreatedAt, &p.ExpiresAt, &p.Offer, &claimedAt)
	if errors.Is(err, sql.ErrNoRows) {
		return PairingSession{}, fmt.Errorf("%w: pairing session", ErrNotFound)
	}
	if err != nil {
		return PairingSession{}, fmt.Errorf("get pairing session: %w", err)
	}
	p.ClaimedAt = scanNullableInt(claimedAt)
	return p, nil
}

// SetPairingOfferWithDevice stores the relay payload for a session that has
// none yet and registers the newly certified device in the same transaction
// (the offer's registration side effect). Returns
// ErrNotFound if the session is absent, ErrConflict if an offer was already
// posted (single-shot) or the device id is already registered; any failure
// rolls back both writes.
func (s *Store) SetPairingOfferWithDevice(ctx context.Context, id string, offer []byte, d Device) error {
	if len(offer) == 0 {
		return fmt.Errorf("%w: pairing offer must not be empty", ErrInvalid)
	}
	if err := validateDevice(d); err != nil {
		return err
	}
	return s.inTx(ctx, func(tx *sql.Tx) error {
		res, err := tx.ExecContext(ctx,
			"UPDATE pairing_session SET offer = ? WHERE id = ? AND offer IS NULL",
			offer, id,
		)
		if err != nil {
			return fmt.Errorf("update pairing session: %w", err)
		}
		n, err := res.RowsAffected()
		if err != nil {
			return fmt.Errorf("update pairing session: %w", err)
		}
		if n == 0 {
			var count int
			err := tx.QueryRowContext(ctx,
				"SELECT COUNT(*) FROM pairing_session WHERE id = ?", id,
			).Scan(&count)
			if err != nil {
				return fmt.Errorf("check pairing session: %w", err)
			}
			if count == 0 {
				return fmt.Errorf("%w: pairing session", ErrNotFound)
			}
			return fmt.Errorf("%w: offer already posted", ErrConflict)
		}
		_, err = tx.ExecContext(ctx, `
INSERT INTO device (id, account_id, name, platform, ed25519_pub, x25519_pub, cert_chain, created_at, revoked_at)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			d.ID, d.AccountID, d.Name, d.Platform, d.Ed25519Pub, d.X25519Pub, d.CertChain, d.CreatedAt, nullableInt(d.RevokedAt),
		)
		if err != nil {
			return mapConstraintErr(err)
		}
		return nil
	})
}

// ClaimPairingSession marks a session claimed. Returns ErrNotFound if the
// session is absent, ErrConflict if it was already claimed (single-use).
// Expiry checks against the TTL are service-layer concerns.
func (s *Store) ClaimPairingSession(ctx context.Context, id string, claimedAt int64) error {
	return s.singleShotUpdate(ctx,
		"UPDATE pairing_session SET claimed_at = ? WHERE id = ? AND claimed_at IS NULL",
		"SELECT COUNT(*) FROM pairing_session WHERE id = ?",
		"pairing session", "session already claimed",
		claimedAt, id,
	)
}

// DeleteExpiredPairingSessions removes sessions whose TTL passed before now.
// Returns the number of sessions removed.
func (s *Store) DeleteExpiredPairingSessions(ctx context.Context, now int64) (int64, error) {
	res, err := s.db.ExecContext(ctx, "DELETE FROM pairing_session WHERE expires_at <= ?", now)
	if err != nil {
		return 0, fmt.Errorf("delete expired pairing sessions: %w", err)
	}
	n, err := res.RowsAffected()
	if err != nil {
		return 0, fmt.Errorf("delete expired pairing sessions: %w", err)
	}
	return n, nil
}

// singleShotUpdate performs a guarded one-time update: the guard is encoded
// in updateSQL's WHERE clause; when no row changes, existsSQL distinguishes
// ErrNotFound from ErrConflict.
func (s *Store) singleShotUpdate(ctx context.Context, updateSQL, existsSQL, entity, conflictMsg string, args ...any) error {
	return s.inTx(ctx, func(tx *sql.Tx) error {
		res, err := tx.ExecContext(ctx, updateSQL, args...)
		if err != nil {
			return fmt.Errorf("update %s: %w", entity, err)
		}
		n, err := res.RowsAffected()
		if err != nil {
			return fmt.Errorf("update %s: %w", entity, err)
		}
		if n > 0 {
			return nil
		}
		var count int
		// The id is always the last argument of the update statement.
		if err := tx.QueryRowContext(ctx, existsSQL, args[len(args)-1]).Scan(&count); err != nil {
			return fmt.Errorf("check %s: %w", entity, err)
		}
		if count == 0 {
			return fmt.Errorf("%w: %s", ErrNotFound, entity)
		}
		return fmt.Errorf("%w: %s", ErrConflict, conflictMsg)
	})
}

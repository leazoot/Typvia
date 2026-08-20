package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
)

// PushResult is the per-record outcome of a batch push: the assigned (or
// pre-existing, for idempotent duplicates) server_seq.
type PushResult struct {
	ID        string
	ServerSeq int64
	// Duplicate is true when the record id was already stored (idempotency):
	// the existing server_seq is returned unchanged.
	Duplicate bool
}

// EntityHead reports the current server head version of one entity in a
// version conflict response (409 carries current heads).
type EntityHead struct {
	EntityType  string
	EntityID    string
	HeadVersion int64
}

// VersionConflictError rejects a batch whose records do not advance their
// entity heads by exactly one (version must equal head + 1). Heads lists
// the current server head for every conflicting entity; nothing from the
// batch is stored.
type VersionConflictError struct {
	Heads []EntityHead
}

// Error implements the error interface without echoing record content.
func (e *VersionConflictError) Error() string {
	return fmt.Sprintf("store: version conflict on %d entities", len(e.Heads))
}

// PushRecords applies a batch atomically: either every non-duplicate record
// is stored (each assigned the next per-account server_seq) or nothing is.
// Records already stored under the same id are idempotent duplicates.
// A record whose version is not exactly the entity head + 1 — counting
// earlier records of the same batch — fails the whole batch with
// *VersionConflictError. All records must belong to accountID.
func (s *Store) PushRecords(ctx context.Context, accountID string, records []Record) ([]PushResult, error) {
	if len(records) == 0 {
		return nil, fmt.Errorf("%w: push requires at least one record", ErrInvalid)
	}
	for _, r := range records {
		if err := validateRecord(r); err != nil {
			return nil, err
		}
		if r.AccountID != accountID {
			return nil, fmt.Errorf("%w: record account mismatch", ErrInvalid)
		}
	}

	results := make([]PushResult, len(records))
	err := s.inTx(ctx, func(tx *sql.Tx) error {
		// Pass 1: resolve idempotent duplicates and current entity heads,
		// then simulate the batch to collect every conflicting entity.
		heads := map[[2]string]int64{}
		simHeads := map[[2]string]int64{}
		var conflicts []EntityHead
		conflicted := map[[2]string]bool{}
		for i, r := range records {
			var existingSeq int64
			scanErr := tx.QueryRowContext(ctx,
				"SELECT server_seq FROM record WHERE id = ?", r.ID,
			).Scan(&existingSeq)
			if scanErr == nil {
				results[i] = PushResult{ID: r.ID, ServerSeq: existingSeq, Duplicate: true}
				continue
			}
			if !errors.Is(scanErr, sql.ErrNoRows) {
				return fmt.Errorf("check record id: %w", scanErr)
			}

			key := [2]string{r.EntityType, r.EntityID}
			if _, ok := heads[key]; !ok {
				head, _, err := headVersionTx(ctx, tx, accountID, r.EntityType, r.EntityID)
				if err != nil {
					return err
				}
				heads[key] = head
				simHeads[key] = head
			}
			if r.Version != simHeads[key]+1 {
				if !conflicted[key] {
					conflicted[key] = true
					conflicts = append(conflicts, EntityHead{
						EntityType:  r.EntityType,
						EntityID:    r.EntityID,
						HeadVersion: heads[key],
					})
				}
				continue
			}
			simHeads[key]++
		}
		if len(conflicts) > 0 {
			return &VersionConflictError{Heads: conflicts}
		}

		// Pass 2: insert non-duplicates in order under monotonic server_seq.
		var next int64
		if err := tx.QueryRowContext(ctx,
			"SELECT COALESCE(MAX(server_seq), 0) FROM record WHERE account_id = ?", accountID,
		).Scan(&next); err != nil {
			return fmt.Errorf("allocate server_seq: %w", err)
		}
		for i, r := range records {
			if results[i].Duplicate {
				continue
			}
			next++
			var ciphertext any
			if !r.isTombstone() {
				ciphertext = r.Ciphertext
			}
			if _, err := tx.ExecContext(ctx, `
INSERT INTO record (id, account_id, entity_type, entity_id, version, ciphertext,
                    deleted_at, updated_at, device_id, key_id, signature, server_seq)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
				r.ID, r.AccountID, r.EntityType, r.EntityID, r.Version, ciphertext,
				nullableInt(r.DeletedAt), r.UpdatedAt, r.DeviceID, r.KeyID, r.Signature, next,
			); err != nil {
				return mapConstraintErr(err)
			}
			results[i] = PushResult{ID: r.ID, ServerSeq: next}
		}
		return nil
	})
	if err != nil {
		return nil, err
	}
	return results, nil
}

func headVersionTx(ctx context.Context, tx *sql.Tx, accountID, entityType, entityID string) (int64, bool, error) {
	var head sql.NullInt64
	err := tx.QueryRowContext(ctx, `
SELECT MAX(version) FROM record
WHERE account_id = ? AND entity_type = ? AND entity_id = ?`,
		accountID, entityType, entityID,
	).Scan(&head)
	if err != nil {
		return 0, false, fmt.Errorf("head version: %w", err)
	}
	return head.Int64, head.Valid, nil
}

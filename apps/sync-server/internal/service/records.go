package service

import (
	"context"
	"crypto/ed25519"
	"errors"
	"fmt"

	"typvia.dev/sync-server/internal/store"
	"typvia.dev/sync-server/internal/wire"
)

// entityTypes is the controlled value set for entity_type.
var entityTypes = map[string]bool{
	"snippet":        true,
	"template_field": true,
	"folder":         true,
	"tag":            true,
	"snippet_tag":    true,
	"app_rule":       true,
	"ai_action":      true,
}

// RecordIn is one pushed sync record in wire form, bytes already decoded
// from base64.
type RecordIn struct {
	ID         string
	EntityType string
	EntityID   string
	Version    int64
	Ciphertext []byte
	DeletedAt  *int64
	UpdatedAt  int64
	DeviceID   string
	KeyID      int64
	Signature  []byte
}

// RecordOut is one pulled sync record including the server cursor.
type RecordOut struct {
	ID         string `json:"id"`
	EntityType string `json:"entity_type"`
	EntityID   string `json:"entity_id"`
	Version    int64  `json:"version"`
	Ciphertext []byte `json:"ciphertext"`
	DeletedAt  *int64 `json:"deleted_at"`
	UpdatedAt  int64  `json:"updated_at"`
	DeviceID   string `json:"device_id"`
	KeyID      int64  `json:"key_id"`
	Signature  []byte `json:"signature"`
	ServerSeq  int64  `json:"server_seq"`
}

// PushResultOut is the per-record outcome of an accepted batch: duplicates
// return their existing server_seq.
type PushResultOut struct {
	ID        string `json:"id"`
	ServerSeq int64  `json:"server_seq"`
	Duplicate bool   `json:"duplicate"`
}

// PullOut is the incremental pull response page.
type PullOut struct {
	Records []RecordOut `json:"records"`
	HasMore bool        `json:"has_more"`
}

// PushRecords admits and stores a batch. Admission checks per record: size
// limits, entity type, tombstone invariant, producing device = authenticated
// device, and the record signature rebuilt server-side. A version that is not
// entity head + 1 rejects the whole batch with VERSION_CONFLICT carrying
// current heads; nothing is stored.
func (s *Service) PushRecords(ctx context.Context, sess Session, records []RecordIn) ([]PushResultOut, error) {
	if len(records) == 0 {
		return nil, MalformedErr("push requires at least one record")
	}
	if len(records) > maxBatchRecords {
		return nil, PayloadTooLargeErr(fmt.Sprintf("batch exceeds %d records", maxBatchRecords))
	}
	device, err := s.activeDevice(ctx, sess.DeviceID, sess.AccountID)
	if err != nil {
		return nil, err
	}

	var totalBytes int
	rows := make([]store.Record, 0, len(records))
	for _, r := range records {
		if err := validateRecordIn(r, sess.DeviceID); err != nil {
			return nil, err
		}
		totalBytes += len(r.Ciphertext)
		if totalBytes > maxBatchBytes {
			return nil, PayloadTooLargeErr(fmt.Sprintf("batch ciphertext exceeds %d bytes", maxBatchBytes))
		}
		signed, err := wire.RecordSignedBytes(wire.RecordFields{
			DeviceID:   r.DeviceID,
			EntityType: r.EntityType,
			EntityID:   r.EntityID,
			Version:    r.Version,
			DeletedAt:  r.DeletedAt,
			UpdatedAt:  r.UpdatedAt,
			Ciphertext: r.Ciphertext,
		})
		if err != nil {
			return nil, MalformedErr("record identity fields must not contain NUL bytes")
		}
		if err := wire.Verify(device.Ed25519Pub, signed, r.Signature); err != nil {
			return nil, MalformedErr("record signature verification failed")
		}
		rows = append(rows, store.Record{
			ID:         r.ID,
			AccountID:  sess.AccountID,
			EntityType: r.EntityType,
			EntityID:   r.EntityID,
			Version:    r.Version,
			Ciphertext: r.Ciphertext,
			DeletedAt:  r.DeletedAt,
			UpdatedAt:  r.UpdatedAt,
			DeviceID:   r.DeviceID,
			KeyID:      r.KeyID,
			Signature:  r.Signature,
		})
	}

	results, err := s.store.PushRecords(ctx, sess.AccountID, rows)
	var conflict *store.VersionConflictError
	if errors.As(err, &conflict) {
		heads := make([]ConflictHead, len(conflict.Heads))
		for i, h := range conflict.Heads {
			heads[i] = ConflictHead{EntityType: h.EntityType, EntityID: h.EntityID, HeadVersion: h.HeadVersion}
		}
		return nil, &Error{
			Kind:      KindBusiness,
			Code:      CodeVersionConflict,
			Message:   "record version is not the entity head + 1; pull, merge and re-push",
			Conflicts: heads,
		}
	}
	if errors.Is(err, store.ErrInvalid) || errors.Is(err, store.ErrConflict) {
		return nil, MalformedErr("batch violates storage invariants")
	}
	if err != nil {
		return nil, systemErr(fmt.Errorf("store batch: %w", err))
	}

	out := make([]PushResultOut, len(results))
	for i, r := range results {
		out[i] = PushResultOut{ID: r.ID, ServerSeq: r.ServerSeq, Duplicate: r.Duplicate}
	}
	return out, nil
}

// PullRecords returns records with server_seq > since in ascending order,
// paged at PullPageLimit.
func (s *Service) PullRecords(ctx context.Context, sess Session, since int64, limit int) (PullOut, error) {
	if since < 0 {
		return PullOut{}, MalformedErr("since must be non-negative")
	}
	if limit <= 0 || limit > PullPageLimit {
		limit = PullPageLimit
	}
	// Fetch one extra row to report has_more without a second query.
	rows, err := s.store.ListRecordsSince(ctx, sess.AccountID, since, limit+1)
	if err != nil {
		return PullOut{}, systemErr(fmt.Errorf("list records: %w", err))
	}
	hasMore := len(rows) > limit
	if hasMore {
		rows = rows[:limit]
	}
	out := make([]RecordOut, len(rows))
	for i, r := range rows {
		out[i] = RecordOut{
			ID:         r.ID,
			EntityType: r.EntityType,
			EntityID:   r.EntityID,
			Version:    r.Version,
			Ciphertext: r.Ciphertext,
			DeletedAt:  r.DeletedAt,
			UpdatedAt:  r.UpdatedAt,
			DeviceID:   r.DeviceID,
			KeyID:      r.KeyID,
			Signature:  r.Signature,
			ServerSeq:  r.ServerSeq,
		}
	}
	return PullOut{Records: out, HasMore: hasMore}, nil
}

// validateRecordIn checks one pushed record's shape before signature
// verification. sessDeviceID is the authenticated pusher: records must be
// self-produced (record signatures bind the producing device).
func validateRecordIn(r RecordIn, sessDeviceID string) error {
	if err := validateIDField("record id", r.ID, maxIDLen); err != nil {
		return err
	}
	if err := validateIDField("entity_id", r.EntityID, maxIDLen); err != nil {
		return err
	}
	if !entityTypes[r.EntityType] {
		return MalformedErr("entity_type is not a known syncable entity")
	}
	if r.Version <= 0 {
		return MalformedErr("version must be positive")
	}
	if r.UpdatedAt <= 0 {
		return MalformedErr("updated_at must be a positive millisecond timestamp")
	}
	if r.DeviceID != sessDeviceID {
		return MalformedErr("record device_id must be the authenticated device")
	}
	if len(r.Signature) != ed25519.SignatureSize {
		return MalformedErr("signature must be a 64-byte Ed25519 signature")
	}
	if r.DeletedAt != nil && len(r.Ciphertext) != 0 {
		return MalformedErr("tombstone must not carry ciphertext")
	}
	if r.DeletedAt == nil && len(r.Ciphertext) == 0 {
		return MalformedErr("non-tombstone must carry ciphertext")
	}
	if len(r.Ciphertext) > maxEnvelopeSize {
		return PayloadTooLargeErr(fmt.Sprintf("record envelope exceeds %d bytes", maxEnvelopeSize))
	}
	return nil
}

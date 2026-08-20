package service

import (
	"context"
	"fmt"

	"typvia.dev/sync-server/internal/store"
)

// maxKeyUpdateSize bounds a sealed key update message (a sealed key bundle,
// small by construction).
const maxKeyUpdateSize = 256 * 1024

// KeyUpdateOut is one sealed key update addressed to the pulling device.
// Payload is opaque sealed bytes.
type KeyUpdateOut struct {
	Seq       int64  `json:"seq"`
	Payload   []byte `json:"payload"`
	CreatedAt int64  `json:"created_at"`
}

// AddKeyUpdate stores a sealed key update for a target device of the same
// account (rotation distribution, late vault authorization). Revoked targets
// are rejected — they must never receive new key material.
func (s *Service) AddKeyUpdate(ctx context.Context, sess Session, targetDeviceID string, payload []byte) (int64, error) {
	if err := validateIDField("target_device_id", targetDeviceID, maxIDLen); err != nil {
		return 0, err
	}
	if len(payload) == 0 {
		return 0, MalformedErr("payload is required")
	}
	if len(payload) > maxKeyUpdateSize {
		return 0, PayloadTooLargeErr(fmt.Sprintf("payload exceeds %d bytes", maxKeyUpdateSize))
	}
	if _, err := s.activeDevice(ctx, targetDeviceID, sess.AccountID); err != nil {
		return 0, err
	}
	seq, err := s.store.AddKeyUpdate(ctx, store.KeyUpdate{
		AccountID:      sess.AccountID,
		TargetDeviceID: targetDeviceID,
		Payload:        payload,
		CreatedAt:      s.nowMillis(),
	})
	if err != nil {
		return 0, systemErr(fmt.Errorf("store key update: %w", err))
	}
	return seq, nil
}

// ListKeyUpdates returns the sealed key updates addressed to the
// authenticated device with seq > since (targeted pull — a device only ever
// sees its own messages). `since` is also the device's acknowledgment:
// everything up to it is dropped first, so a message this device has
// consumed stops occupying the relay. A failing cleanup surfaces as a
// system error rather than being swallowed: the same storage would be
// serving the pull that follows it.
func (s *Service) ListKeyUpdates(ctx context.Context, sess Session, since int64) ([]KeyUpdateOut, error) {
	if since < 0 {
		return nil, MalformedErr("since must be non-negative")
	}
	if _, err := s.store.DeleteKeyUpdatesThrough(ctx, sess.AccountID, sess.DeviceID, since); err != nil {
		return nil, systemErr(fmt.Errorf("prune key updates: %w", err))
	}
	updates, err := s.store.ListKeyUpdatesSince(ctx, sess.AccountID, sess.DeviceID, since)
	if err != nil {
		return nil, systemErr(fmt.Errorf("list key updates: %w", err))
	}
	out := make([]KeyUpdateOut, len(updates))
	for i, u := range updates {
		out[i] = KeyUpdateOut{Seq: u.Seq, Payload: u.Payload, CreatedAt: u.CreatedAt}
	}
	return out, nil
}

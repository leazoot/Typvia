// Package service holds the business layer of the sync server: protocol
// negotiation and the error taxonomy handlers map to HTTP responses. It
// contains no SQL (store's job) and no HTTP concerns (handler's job).
package service

import (
	"context"
	"errors"
	"fmt"
	"strconv"
	"strings"
	"time"

	"typvia.dev/sync-server/internal/store"
)

// ProtocolMin and ProtocolMax bound the supported sync protocol versions
// (currently protocol_version = 1).
const (
	ProtocolMin = 1
	ProtocolMax = 1
)

// ServerVersion identifies this server build in the handshake response.
const ServerVersion = "0.1.0"

// Protocol lifetimes and limits.
const (
	challengeTTL = 60 * time.Second
	sessionTTL   = 15 * time.Minute
	pairingTTL   = 10 * time.Minute

	maxBatchRecords = 500
	maxEnvelopeSize = 256 * 1024
	maxBatchBytes   = 4 * 1024 * 1024
	// PullPageLimit is the default and maximum pull page size.
	PullPageLimit = 500

	// maxIDLen / maxNameLen bound variable-length identity fields on input.
	maxIDLen   = 128
	maxNameLen = 200
)

// Service exposes the sync server use cases to handlers.
type Service struct {
	store    *store.Store
	sessions *sessionStore
	now      func() time.Time
}

// Option customizes a Service (test seams only).
type Option func(*Service)

// WithClock replaces the wall clock, letting tests drive TTL expiry.
func WithClock(now func() time.Time) Option {
	return func(s *Service) { s.now = now }
}

// New builds a Service on top of the persistence layer.
func New(st *store.Store, opts ...Option) *Service {
	s := &Service{store: st, sessions: newSessionStore(), now: time.Now}
	for _, opt := range opts {
		opt(s)
	}
	return s
}

// nowMillis is the current time as milliseconds UTC (the protocol timestamp
// unit).
func (s *Service) nowMillis() int64 {
	return s.now().UnixMilli()
}

// validateIDField checks a variable-length identity field: non-blank, length
// bounded, and free of NUL bytes (the separator encoding forbids them).
func validateIDField(name, v string, maxLen int) *Error {
	if strings.TrimSpace(v) == "" {
		return MalformedErr(name + " is required")
	}
	if len(v) > maxLen {
		return MalformedErr(fmt.Sprintf("%s exceeds %d bytes", name, maxLen))
	}
	if strings.ContainsRune(v, 0x00) {
		return MalformedErr(name + " must not contain NUL bytes")
	}
	return nil
}

// activeDevice loads a device, requiring it to belong to accountID (empty =
// any account) and to not be revoked.
func (s *Service) activeDevice(ctx context.Context, deviceID, accountID string) (store.Device, error) {
	d, err := s.store.GetDevice(ctx, deviceID)
	if errors.Is(err, store.ErrNotFound) {
		return store.Device{}, NotFoundErr("device is not registered")
	}
	if err != nil {
		return store.Device{}, systemErr(fmt.Errorf("load device: %w", err))
	}
	if accountID != "" && d.AccountID != accountID {
		// A device of another account is indistinguishable from an unknown
		// one: no cross-account existence leak.
		return store.Device{}, NotFoundErr("device is not registered")
	}
	if d.RevokedAt != nil {
		return store.Device{}, businessErr(CodeDeviceRevoked, "device has been revoked")
	}
	return d, nil
}

// HandshakeInfo is the handshake response body.
type HandshakeInfo struct {
	ProtocolMin   int    `json:"protocol_min"`
	ProtocolMax   int    `json:"protocol_max"`
	ServerVersion string `json:"server_version"`
}

// Handshake validates the client protocol header and returns the server
// protocol window. protocolHeader is the raw X-Typvia-Protocol value; an
// empty string (header absent) is allowed — handshake doubles as discovery
// for clients that do not yet know the server window. A present header must
// parse as an integer within [ProtocolMin, ProtocolMax]; anything else is
// PROTOCOL_UNSUPPORTED — no downgrade guessing. The error message never
// echoes the client-supplied value.
func (s *Service) Handshake(protocolHeader string) (HandshakeInfo, error) {
	info := HandshakeInfo{
		ProtocolMin:   ProtocolMin,
		ProtocolMax:   ProtocolMax,
		ServerVersion: ServerVersion,
	}
	if protocolHeader == "" {
		return info, nil
	}
	v, err := strconv.Atoi(protocolHeader)
	if err != nil || v < ProtocolMin || v > ProtocolMax {
		return HandshakeInfo{}, protocolErr(CodeProtocolUnsupported,
			fmt.Sprintf("protocol version not supported; server supports %d..%d", ProtocolMin, ProtocolMax))
	}
	return info, nil
}

// Health reports whether the persistence layer is reachable.
func (s *Service) Health(ctx context.Context) error {
	if err := s.store.Ping(ctx); err != nil {
		return systemErr(fmt.Errorf("database unreachable: %w", err))
	}
	return nil
}

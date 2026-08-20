package service

import (
	"context"
	"crypto/ed25519"
	"encoding/json"
	"errors"
	"fmt"

	"typvia.dev/sync-server/internal/store"
	"typvia.dev/sync-server/internal/wire"
)

// RootDeviceInput is a self-signed root statement submission: account
// creation or recovery re-root.
type RootDeviceInput struct {
	DeviceID   string
	Ed25519Pub []byte
	X25519Pub  []byte
	Name       string
	Platform   string
	CreatedAt  int64
	// Signature is the Ed25519 self-signature over root_signed_bytes.
	Signature []byte
}

// rootStatementDoc is the stored (and directory-served) JSON form of a root
// statement. Fields mirror the signed subject set; []byte fields serialize as
// base64.
type rootStatementDoc struct {
	DeviceID   string `json:"device_id"`
	Ed25519Pub []byte `json:"ed25519_pub"`
	X25519Pub  []byte `json:"x25519_pub"`
	Name       string `json:"name"`
	Platform   string `json:"platform"`
	CreatedAt  int64  `json:"created_at"`
	Signature  []byte `json:"signature"`
}

// emptyCertChain is the stored chain of a root device: the root statement is
// its identity, no certificate chain exists.
var emptyCertChain = []byte("[]")

// validateRootDevice checks field shapes and verifies the self-signature
// over root_signed_bytes. Admission filter only — clients remain the
// verification authority.
func validateRootDevice(in RootDeviceInput) error {
	if err := validateIDField("device_id", in.DeviceID, maxIDLen); err != nil {
		return err
	}
	if err := validateIDField("name", in.Name, maxNameLen); err != nil {
		return err
	}
	if err := validateIDField("platform", in.Platform, maxIDLen); err != nil {
		return err
	}
	if len(in.Ed25519Pub) != ed25519.PublicKeySize || len(in.X25519Pub) != 32 {
		return MalformedErr("public keys must be 32 bytes")
	}
	if in.CreatedAt <= 0 {
		return MalformedErr("created_at must be a positive millisecond timestamp")
	}
	signed, err := wire.RootSignedBytes(wire.RootSubject{
		DeviceID:   in.DeviceID,
		Ed25519Pub: in.Ed25519Pub,
		X25519Pub:  in.X25519Pub,
		Name:       in.Name,
		Platform:   in.Platform,
		CreatedAt:  in.CreatedAt,
	})
	if err != nil {
		return MalformedErr("root statement fields must not contain NUL bytes")
	}
	if err := wire.Verify(in.Ed25519Pub, signed, in.Signature); err != nil {
		return MalformedErr("root statement signature verification failed")
	}
	return nil
}

// rootDeviceRow converts a verified root statement input to its device row
// and stored statement bytes.
func rootDeviceRow(accountID string, in RootDeviceInput, now int64) (store.Device, []byte, error) {
	statement, err := json.Marshal(rootStatementDoc{
		DeviceID:   in.DeviceID,
		Ed25519Pub: in.Ed25519Pub,
		X25519Pub:  in.X25519Pub,
		Name:       in.Name,
		Platform:   in.Platform,
		CreatedAt:  in.CreatedAt,
		Signature:  in.Signature,
	})
	if err != nil {
		return store.Device{}, nil, systemErr(fmt.Errorf("encode root statement: %w", err))
	}
	return store.Device{
		ID:         in.DeviceID,
		AccountID:  accountID,
		Name:       in.Name,
		Platform:   in.Platform,
		Ed25519Pub: in.Ed25519Pub,
		X25519Pub:  in.X25519Pub,
		CertChain:  emptyCertChain,
		CreatedAt:  now,
	}, statement, nil
}

// CreateAccount registers a new account from its first device's self-signed
// root statement. The server verifies the self-signature as admission and
// assigns the account id.
func (s *Service) CreateAccount(ctx context.Context, in RootDeviceInput) (string, error) {
	if err := validateRootDevice(in); err != nil {
		return "", err
	}
	accountID, err := newUUID()
	if err != nil {
		return "", systemErr(fmt.Errorf("generate account id: %w", err))
	}
	now := s.nowMillis()
	device, statement, err := rootDeviceRow(accountID, in, now)
	if err != nil {
		return "", err
	}
	err = s.store.CreateAccountWithDevice(ctx,
		store.Account{ID: accountID, RootStatement: statement, CreatedAt: now}, device)
	if errors.Is(err, store.ErrConflict) {
		return "", MalformedErr("device is already registered")
	}
	if err != nil {
		return "", systemErr(fmt.Errorf("create account: %w", err))
	}
	return accountID, nil
}

// newUUID returns a random (version 4) UUID from the CSPRNG.
func newUUID() (string, error) {
	b, err := newSecret(16)
	if err != nil {
		return "", err
	}
	b[6] = (b[6] & 0x0f) | 0x40
	b[8] = (b[8] & 0x3f) | 0x80
	return fmt.Sprintf("%x-%x-%x-%x-%x", b[0:4], b[4:6], b[6:8], b[8:10], b[10:16]), nil
}

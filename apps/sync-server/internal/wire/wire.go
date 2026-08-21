// Typvia sync server
// Copyright (C) 2026 Typvia contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at
// your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero
// General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

// Package wire builds the signed-byte messages of the sync protocol that the
// server verifies as an admission filter. Only public-key verification
// happens here: the server holds no private keys and clients remain the
// verification authority.
package wire

import (
	"bytes"
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/binary"
	"errors"
	"fmt"
)

// Domain-separation prefixes, one per signed message type.
const (
	rootContext      = "typvia.root.v1"
	certContext      = "typvia.devcert.v1"
	recordContext    = "typvia.syncrec.v1"
	authContext      = "typvia.auth.v1"
	revokeContext    = "typvia.revoke.v1"
	rootProofContext = "typvia.rootproof.v1"
)

// noDeletedAt is the deleted_at placeholder for non-tombstone records in the
// record signature (0xFFFFFFFFFFFFFFFF).
const noDeletedAt = ^uint64(0)

// ErrEmbeddedNul reports a variable-length field containing a 0x00 byte,
// which the separator encoding cannot represent unambiguously.
var ErrEmbeddedNul = errors.New("wire: variable-length field contains NUL byte")

// RootSubject is the content set covered by a root statement signature.
type RootSubject struct {
	DeviceID   string
	Ed25519Pub []byte
	X25519Pub  []byte
	Name       string
	Platform   string
	CreatedAt  int64
}

// RootSignedBytes builds root_signed_bytes: the self-signed root
// statement message of the account's first device.
func RootSignedBytes(s RootSubject) ([]byte, error) {
	var b bytes.Buffer
	b.WriteString(rootContext)
	if err := writeVar(&b, s.DeviceID); err != nil {
		return nil, err
	}
	b.Write(s.Ed25519Pub)
	b.Write(s.X25519Pub)
	if err := writeVar(&b, s.Name); err != nil {
		return nil, err
	}
	if err := writeVar(&b, s.Platform); err != nil {
		return nil, err
	}
	writeLE64(&b, uint64(s.CreatedAt))
	return b.Bytes(), nil
}

// CertSignedBytes builds cert_signed_bytes: the device certificate
// message an admitting device signs at pairing time. The subject set is the
// root subject set; issued_at and the issuer id bind the chain structure.
func CertSignedBytes(s RootSubject, issuedAt int64, issuerDeviceID string) ([]byte, error) {
	var b bytes.Buffer
	b.WriteString(certContext)
	if err := writeVar(&b, s.DeviceID); err != nil {
		return nil, err
	}
	b.Write(s.Ed25519Pub)
	b.Write(s.X25519Pub)
	if err := writeVar(&b, s.Name); err != nil {
		return nil, err
	}
	if err := writeVar(&b, s.Platform); err != nil {
		return nil, err
	}
	writeLE64(&b, uint64(s.CreatedAt))
	writeLE64(&b, uint64(issuedAt))
	if err := writeVar(&b, issuerDeviceID); err != nil {
		return nil, err
	}
	return b.Bytes(), nil
}

// RecordFields identifies one sync record for signature reconstruction.
// Ciphertext is empty for tombstones.
type RecordFields struct {
	DeviceID   string
	EntityType string
	EntityID   string
	Version    int64
	// DeletedAt is nil for non-tombstones.
	DeletedAt  *int64
	UpdatedAt  int64
	Ciphertext []byte
}

// RecordSignedBytes builds signed_bytes for one sync record.
func RecordSignedBytes(r RecordFields) ([]byte, error) {
	var b bytes.Buffer
	b.WriteString(recordContext)
	if err := writeVar(&b, r.DeviceID); err != nil {
		return nil, err
	}
	if err := writeVar(&b, r.EntityType); err != nil {
		return nil, err
	}
	if err := writeVar(&b, r.EntityID); err != nil {
		return nil, err
	}
	writeLE64(&b, uint64(r.Version))
	if r.DeletedAt != nil {
		writeLE64(&b, uint64(*r.DeletedAt))
	} else {
		writeLE64(&b, noDeletedAt)
	}
	writeLE64(&b, uint64(r.UpdatedAt))
	digest := sha256.Sum256(r.Ciphertext)
	b.Write(digest[:])
	return b.Bytes(), nil
}

// AuthSignedBytes builds the challenge-response message:
// "typvia.auth.v1" || challenge || device_id.
func AuthSignedBytes(challenge []byte, deviceID string) []byte {
	var b bytes.Buffer
	b.WriteString(authContext)
	b.Write(challenge)
	b.WriteString(deviceID)
	return b.Bytes()
}

// RevokeSignedBytes builds the revocation statement message:
// "typvia.revoke.v1" || revoked_device_id || revoked_at(8B LE). The trailing
// field is fixed-length, so no separator is needed.
func RevokeSignedBytes(revokedDeviceID string, revokedAt int64) []byte {
	var b bytes.Buffer
	b.WriteString(revokeContext)
	b.WriteString(revokedDeviceID)
	writeLE64(&b, uint64(revokedAt))
	return b.Bytes()
}

// RootProofSignedBytes builds the MK-possession proof message for recovery
// re-root: "typvia.rootproof.v1" || challenge(32B) || account_id.
func RootProofSignedBytes(challenge []byte, accountID string) []byte {
	var b bytes.Buffer
	b.WriteString(rootProofContext)
	b.Write(challenge)
	b.WriteString(accountID)
	return b.Bytes()
}

// Verify checks an Ed25519 signature over message with a 32-byte public key.
// Go's ed25519.Verify rejects non-canonical scalar encodings; this is an
// admission filter only; client verification is authoritative.
func Verify(publicKey, message, signature []byte) error {
	if len(publicKey) != ed25519.PublicKeySize {
		return fmt.Errorf("wire: public key must be %d bytes", ed25519.PublicKeySize)
	}
	if len(signature) != ed25519.SignatureSize {
		return fmt.Errorf("wire: signature must be %d bytes", ed25519.SignatureSize)
	}
	if !ed25519.Verify(ed25519.PublicKey(publicKey), message, signature) {
		return errors.New("wire: signature verification failed")
	}
	return nil
}

// writeVar appends a variable-length field with its 0x00 terminator,
// rejecting embedded NUL bytes.
func writeVar(b *bytes.Buffer, s string) error {
	if bytes.IndexByte([]byte(s), 0x00) >= 0 {
		return ErrEmbeddedNul
	}
	b.WriteString(s)
	b.WriteByte(0x00)
	return nil
}

func writeLE64(b *bytes.Buffer, v uint64) {
	var buf [8]byte
	binary.LittleEndian.PutUint64(buf[:], v)
	b.Write(buf[:])
}

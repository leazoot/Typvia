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

// maxPairingOfferSize bounds the sealed key bundle of an offer: far above
// any legitimate bundle, far below abuse sizes.
const maxPairingOfferSize = 256 * 1024

// PairingBeginOut is the pairing session creation response (step 1).
type PairingBeginOut struct {
	SessionID string
	// ExpiresIn is the session TTL in seconds.
	ExpiresIn int
}

// PairingClaimOut is the claim response: Pending until the trusted device
// posts its offer, then the opaque relay payload exactly once.
type PairingClaimOut struct {
	Pending bool
	Payload []byte
}

// errPairingUnavailable is the uniform rejection for absent, expired,
// already-offered and already-claimed sessions: one non-leaky answer for
// every unusable state (session ids transit QR codes; no state probing).
func errPairingUnavailable() *Error {
	return NotFoundErr("pairing session is not available")
}

// BeginPairing creates a pairing relay session (TTL 10 min). The
// caller holds account_id from the pairing code; no device session exists
// yet on the new device.
func (s *Service) BeginPairing(ctx context.Context, accountID string) (PairingBeginOut, error) {
	if err := validateIDField("account_id", accountID, maxIDLen); err != nil {
		return PairingBeginOut{}, err
	}
	if _, err := s.store.GetAccount(ctx, accountID); errors.Is(err, store.ErrNotFound) {
		return PairingBeginOut{}, NotFoundErr("account is not registered")
	} else if err != nil {
		return PairingBeginOut{}, systemErr(fmt.Errorf("load account: %w", err))
	}
	id, err := newUUID()
	if err != nil {
		return PairingBeginOut{}, systemErr(fmt.Errorf("generate session id: %w", err))
	}
	now := s.nowMillis()
	err = s.store.CreatePairingSession(ctx, store.PairingSession{
		ID:        id,
		AccountID: accountID,
		CreatedAt: now,
		ExpiresAt: now + pairingTTL.Milliseconds(),
	})
	if err != nil {
		return PairingBeginOut{}, systemErr(fmt.Errorf("create pairing session: %w", err))
	}
	return PairingBeginOut{SessionID: id, ExpiresIn: int(pairingTTL.Seconds())}, nil
}

// PairingCertificateInput is the new device's certificate as posted by the
// admitting device (the signed subject set plus issuance fields).
type PairingCertificateInput struct {
	DeviceID       string
	Ed25519Pub     []byte
	X25519Pub      []byte
	Name           string
	Platform       string
	CreatedAt      int64
	IssuedAt       int64
	IssuerDeviceID string
	// Signature is the issuer's Ed25519 signature over cert_signed_bytes.
	Signature []byte
}

// certDoc is the stored JSON form of one certificate chain link, mirroring
// the client-side directory encoding ([]byte fields serialize as base64).
type certDoc struct {
	DeviceID       string `json:"device_id"`
	Ed25519Pub     []byte `json:"ed25519_pub"`
	X25519Pub      []byte `json:"x25519_pub"`
	Name           string `json:"name"`
	Platform       string `json:"platform"`
	CreatedAt      int64  `json:"created_at"`
	IssuedAt       int64  `json:"issued_at"`
	IssuerDeviceID string `json:"issuer_device_id"`
	Signature      []byte `json:"signature"`
}

// claimDoc is the relay payload composed at offer time and handed out by
// claim: trust root, full certificate chain, and the sealed key bundle.
type claimDoc struct {
	RootStatement   json.RawMessage `json:"root_statement"`
	CertChain       json.RawMessage `json:"cert_chain"`
	SealedBundle    []byte          `json:"sealed_bundle"`
	BundleSignature []byte          `json:"bundle_signature"`
}

// PostPairingOffer admits the new device: the certificate is verified
// against the authenticated admitting device's key (admission filter —
// clients remain the verification authority), then in one transaction the
// offer is stored and the new device registered with chain = admitting
// device's chain + the new certificate. Single-shot.
// The sealed bundle stays opaque: only ciphertext and public material
// transit the server.
func (s *Service) PostPairingOffer(ctx context.Context, sess Session, sessionID string, cert PairingCertificateInput, sealedBundle, bundleSig []byte) error {
	if err := validateIDField("session id", sessionID, maxIDLen); err != nil {
		return err
	}
	if len(sealedBundle) == 0 {
		return MalformedErr("sealed_bundle is required")
	}
	if len(sealedBundle) > maxPairingOfferSize {
		return PayloadTooLargeErr(fmt.Sprintf("sealed_bundle exceeds %d bytes", maxPairingOfferSize))
	}
	if len(bundleSig) != ed25519.SignatureSize {
		return MalformedErr("bundle_signature must be a 64-byte Ed25519 signature")
	}
	p, err := s.pairingSessionInState(ctx, sessionID)
	if err != nil {
		return err
	}
	if p.AccountID != sess.AccountID {
		return errPairingUnavailable()
	}
	issuer, err := s.activeDevice(ctx, sess.DeviceID, sess.AccountID)
	if err != nil {
		return err
	}
	if err := validatePairingCertificate(cert, issuer); err != nil {
		return err
	}

	chain, err := appendCertToChain(issuer.CertChain, cert)
	if err != nil {
		return err
	}
	account, err := s.store.GetAccount(ctx, sess.AccountID)
	if err != nil {
		return systemErr(fmt.Errorf("load account: %w", err))
	}
	payload, err := json.Marshal(claimDoc{
		RootStatement:   json.RawMessage(account.RootStatement),
		CertChain:       json.RawMessage(chain),
		SealedBundle:    sealedBundle,
		BundleSignature: bundleSig,
	})
	if err != nil {
		return systemErr(fmt.Errorf("encode claim payload: %w", err))
	}

	err = s.store.SetPairingOfferWithDevice(ctx, sessionID, payload, store.Device{
		ID:         cert.DeviceID,
		AccountID:  sess.AccountID,
		Name:       cert.Name,
		Platform:   cert.Platform,
		Ed25519Pub: cert.Ed25519Pub,
		X25519Pub:  cert.X25519Pub,
		CertChain:  chain,
		CreatedAt:  s.nowMillis(),
	})
	if errors.Is(err, store.ErrConflict) || errors.Is(err, store.ErrNotFound) {
		// Session gone/used, or the device id raced into existence: one
		// uniform non-leaky answer (session ids transit QR codes).
		return errPairingUnavailable()
	}
	if err != nil {
		return systemErr(fmt.Errorf("store pairing offer: %w", err))
	}
	return nil
}

// validatePairingCertificate checks field shapes, chain binding to the
// authenticated issuer, and the issuer's signature over cert_signed_bytes.
func validatePairingCertificate(cert PairingCertificateInput, issuer store.Device) *Error {
	if err := validateIDField("device_id", cert.DeviceID, maxIDLen); err != nil {
		return err
	}
	if err := validateIDField("name", cert.Name, maxNameLen); err != nil {
		return err
	}
	if err := validateIDField("platform", cert.Platform, maxIDLen); err != nil {
		return err
	}
	if len(cert.Ed25519Pub) != ed25519.PublicKeySize || len(cert.X25519Pub) != 32 {
		return MalformedErr("public keys must be 32 bytes")
	}
	if cert.CreatedAt <= 0 || cert.IssuedAt <= 0 {
		return MalformedErr("certificate timestamps must be positive millisecond values")
	}
	if cert.IssuerDeviceID != issuer.ID {
		return MalformedErr("certificate issuer must be the authenticated device")
	}
	signed, err := wire.CertSignedBytes(wire.RootSubject{
		DeviceID:   cert.DeviceID,
		Ed25519Pub: cert.Ed25519Pub,
		X25519Pub:  cert.X25519Pub,
		Name:       cert.Name,
		Platform:   cert.Platform,
		CreatedAt:  cert.CreatedAt,
	}, cert.IssuedAt, cert.IssuerDeviceID)
	if err != nil {
		return MalformedErr("certificate fields must not contain NUL bytes")
	}
	if err := wire.Verify(issuer.Ed25519Pub, signed, cert.Signature); err != nil {
		return MalformedErr("certificate signature verification failed")
	}
	return nil
}

// appendCertToChain extends the issuer's stored chain (root-first JSON
// array) with the newly issued certificate. The error result is the
// interface type on purpose: a typed-nil *Error assigned into a reused
// `err` variable would read as non-nil.
func appendCertToChain(issuerChain []byte, cert PairingCertificateInput) ([]byte, error) {
	var links []json.RawMessage
	if err := json.Unmarshal(issuerChain, &links); err != nil {
		return nil, systemErr(fmt.Errorf("decode issuer chain: %w", err))
	}
	leaf, err := json.Marshal(certDoc{
		DeviceID:       cert.DeviceID,
		Ed25519Pub:     cert.Ed25519Pub,
		X25519Pub:      cert.X25519Pub,
		Name:           cert.Name,
		Platform:       cert.Platform,
		CreatedAt:      cert.CreatedAt,
		IssuedAt:       cert.IssuedAt,
		IssuerDeviceID: cert.IssuerDeviceID,
		Signature:      cert.Signature,
	})
	if err != nil {
		return nil, systemErr(fmt.Errorf("encode certificate: %w", err))
	}
	links = append(links, leaf)
	chain, err := json.Marshal(links)
	if err != nil {
		return nil, systemErr(fmt.Errorf("encode chain: %w", err))
	}
	return chain, nil
}

// ClaimPairing hands the relay payload to the new device: pending while no
// offer exists, then single-use — the first successful
// claim burns the session.
func (s *Service) ClaimPairing(ctx context.Context, sessionID string) (PairingClaimOut, error) {
	if err := validateIDField("session id", sessionID, maxIDLen); err != nil {
		return PairingClaimOut{}, err
	}
	p, err := s.pairingSessionInState(ctx, sessionID)
	if err != nil {
		return PairingClaimOut{}, err
	}
	if p.Offer == nil {
		return PairingClaimOut{Pending: true}, nil
	}
	err = s.store.ClaimPairingSession(ctx, sessionID, s.nowMillis())
	if errors.Is(err, store.ErrConflict) || errors.Is(err, store.ErrNotFound) {
		return PairingClaimOut{}, errPairingUnavailable()
	}
	if err != nil {
		return PairingClaimOut{}, systemErr(fmt.Errorf("claim pairing session: %w", err))
	}
	return PairingClaimOut{Payload: p.Offer}, nil
}

// pairingSessionInState loads a session that is neither expired nor claimed.
func (s *Service) pairingSessionInState(ctx context.Context, sessionID string) (store.PairingSession, error) {
	p, err := s.store.GetPairingSession(ctx, sessionID)
	if errors.Is(err, store.ErrNotFound) {
		return store.PairingSession{}, errPairingUnavailable()
	}
	if err != nil {
		return store.PairingSession{}, systemErr(fmt.Errorf("load pairing session: %w", err))
	}
	if s.nowMillis() >= p.ExpiresAt || p.ClaimedAt != nil {
		return store.PairingSession{}, errPairingUnavailable()
	}
	return p, nil
}

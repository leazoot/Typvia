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
	"fmt"

	"typvia.dev/sync-server/internal/wire"
)

// ChallengeOut is the auth challenge response (step 1).
type ChallengeOut struct {
	// Challenge is 32 CSPRNG bytes, one-time, short TTL.
	Challenge []byte
	// ExpiresIn is the challenge lifetime in seconds.
	ExpiresIn int
}

// SessionOut is the session token response (step 2).
type SessionOut struct {
	SessionToken string
	// ExpiresIn is the token lifetime in seconds.
	ExpiresIn int
}

// IssueChallenge starts the challenge–response exchange for a device.
// Revoked devices are rejected at this stage already.
func (s *Service) IssueChallenge(ctx context.Context, deviceID string) (ChallengeOut, error) {
	if err := validateIDField("device_id", deviceID, maxIDLen); err != nil {
		return ChallengeOut{}, err
	}
	if _, err := s.activeDevice(ctx, deviceID, ""); err != nil {
		return ChallengeOut{}, err
	}
	value, err := newSecret(32)
	if err != nil {
		return ChallengeOut{}, systemErr(fmt.Errorf("generate challenge: %w", err))
	}
	s.sessions.putAuthChallenge(deviceID, value, s.now().Add(challengeTTL))
	return ChallengeOut{Challenge: value, ExpiresIn: int(challengeTTL.Seconds())}, nil
}

// CreateSession exchanges a signed challenge for a session token (step 2).
// The challenge is consumed on the attempt whatever its outcome
// (one-time, burned on use); the signature covers
// "typvia.auth.v1" || challenge || device_id.
func (s *Service) CreateSession(ctx context.Context, deviceID string, sig []byte) (SessionOut, error) {
	if err := validateIDField("device_id", deviceID, maxIDLen); err != nil {
		return SessionOut{}, err
	}
	if len(sig) != ed25519.SignatureSize {
		return SessionOut{}, MalformedErr("sig must be a 64-byte Ed25519 signature")
	}
	challengeValue, ok := s.sessions.takeAuthChallenge(deviceID, s.now())
	if !ok {
		return SessionOut{}, businessErr(CodeAuthChallengeExpired, "no valid challenge for this device; request a new one")
	}
	device, err := s.activeDevice(ctx, deviceID, "")
	if err != nil {
		return SessionOut{}, err
	}
	if err := wire.Verify(device.Ed25519Pub, wire.AuthSignedBytes(challengeValue, deviceID), sig); err != nil {
		return SessionOut{}, MalformedErr("challenge signature verification failed")
	}
	token, err := s.sessions.createSession(
		Session{DeviceID: device.ID, AccountID: device.AccountID}, s.now().Add(sessionTTL), s.now())
	if err != nil {
		return SessionOut{}, systemErr(fmt.Errorf("create session: %w", err))
	}
	return SessionOut{SessionToken: token, ExpiresIn: int(sessionTTL.Seconds())}, nil
}

// Authenticate resolves a bearer token to its device session. The device's
// revocation state is re-checked on every request so a revocation takes
// effect within the token lifetime.
func (s *Service) Authenticate(ctx context.Context, token string) (Session, error) {
	if token == "" {
		return Session{}, businessErr(CodeSessionExpired, "missing bearer token")
	}
	sess, ok := s.sessions.getSession(token, s.now())
	if !ok {
		return Session{}, businessErr(CodeSessionExpired, "session is expired or unknown")
	}
	if _, err := s.activeDevice(ctx, sess.DeviceID, sess.AccountID); err != nil {
		return Session{}, err
	}
	return sess, nil
}

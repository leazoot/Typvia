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
	"crypto/rand"
	"encoding/base64"
	"fmt"
	"sync"
	"time"
)

// Session identifies an authenticated device. The token behind it is a
// transport-layer admission credential only and carries no cryptographic
// trust.
type Session struct {
	DeviceID  string
	AccountID string
}

// challenge is a pending one-time value: an auth challenge (keyed by device
// id) or a re-root proof challenge (keyed by account id).
type challenge struct {
	value     []byte
	expiresAt time.Time
}

type sessionEntry struct {
	session   Session
	expiresAt time.Time
}

// sessionStore keeps challenges and session tokens in memory. Deliberate
// trade-off: tokens are short-lived admission credentials — losing them on
// restart only forces devices through a fresh challenge exchange, so durable
// storage would add persistence of secret-adjacent material for no security
// or availability gain.
type sessionStore struct {
	mu sync.Mutex
	// authChallenges is keyed by device id: issuing a new challenge replaces
	// the previous one, so one challenge is outstanding per device.
	authChallenges map[string]challenge
	// rootChallenges is keyed by account id (re-root proof).
	rootChallenges map[string]challenge
	// sessions is keyed by token.
	sessions map[string]sessionEntry
}

func newSessionStore() *sessionStore {
	return &sessionStore{
		authChallenges: map[string]challenge{},
		rootChallenges: map[string]challenge{},
		sessions:       map[string]sessionEntry{},
	}
}

// newSecret returns n cryptographically random bytes.
func newSecret(n int) ([]byte, error) {
	b := make([]byte, n)
	if _, err := rand.Read(b); err != nil {
		return nil, fmt.Errorf("read csprng: %w", err)
	}
	return b, nil
}

func (st *sessionStore) putAuthChallenge(deviceID string, value []byte, expiresAt time.Time) {
	st.mu.Lock()
	defer st.mu.Unlock()
	st.authChallenges[deviceID] = challenge{value: value, expiresAt: expiresAt}
}

// takeAuthChallenge removes and returns the pending challenge for a device.
// The challenge is consumed regardless of what the caller does next
// (one-time, burned on use).
func (st *sessionStore) takeAuthChallenge(deviceID string, now time.Time) ([]byte, bool) {
	st.mu.Lock()
	defer st.mu.Unlock()
	c, ok := st.authChallenges[deviceID]
	if !ok {
		return nil, false
	}
	delete(st.authChallenges, deviceID)
	if now.After(c.expiresAt) {
		return nil, false
	}
	return c.value, true
}

func (st *sessionStore) putRootChallenge(accountID string, value []byte, expiresAt time.Time) {
	st.mu.Lock()
	defer st.mu.Unlock()
	st.rootChallenges[accountID] = challenge{value: value, expiresAt: expiresAt}
}

func (st *sessionStore) takeRootChallenge(accountID string, now time.Time) ([]byte, bool) {
	st.mu.Lock()
	defer st.mu.Unlock()
	c, ok := st.rootChallenges[accountID]
	if !ok {
		return nil, false
	}
	delete(st.rootChallenges, accountID)
	if now.After(c.expiresAt) {
		return nil, false
	}
	return c.value, true
}

// prunePastSessions caps memory: once the session map grows large, expired
// entries are swept before a new one is added.
const prunePastSessions = 4096

// createSession mints an opaque token for the device.
func (st *sessionStore) createSession(s Session, expiresAt time.Time, now time.Time) (string, error) {
	raw, err := newSecret(32)
	if err != nil {
		return "", err
	}
	token := base64.RawURLEncoding.EncodeToString(raw)
	st.mu.Lock()
	defer st.mu.Unlock()
	if len(st.sessions) >= prunePastSessions {
		for k, e := range st.sessions {
			if now.After(e.expiresAt) {
				delete(st.sessions, k)
			}
		}
	}
	st.sessions[token] = sessionEntry{session: s, expiresAt: expiresAt}
	return token, nil
}

// getSession resolves a token, dropping it when expired.
func (st *sessionStore) getSession(token string, now time.Time) (Session, bool) {
	st.mu.Lock()
	defer st.mu.Unlock()
	e, ok := st.sessions[token]
	if !ok {
		return Session{}, false
	}
	if now.After(e.expiresAt) {
		delete(st.sessions, token)
		return Session{}, false
	}
	return e.session, true
}

// dropDevice invalidates every session and pending challenge of one device
// (revocation execution).
func (st *sessionStore) dropDevice(deviceID string) {
	st.mu.Lock()
	defer st.mu.Unlock()
	delete(st.authChallenges, deviceID)
	for token, e := range st.sessions {
		if e.session.DeviceID == deviceID {
			delete(st.sessions, token)
		}
	}
}

// dropAccount invalidates every session of an account plus the pending
// challenges of the listed devices (recovery re-root: all previous devices
// lose access).
func (st *sessionStore) dropAccount(accountID string, deviceIDs []string) {
	st.mu.Lock()
	defer st.mu.Unlock()
	delete(st.rootChallenges, accountID)
	for _, id := range deviceIDs {
		delete(st.authChallenges, id)
	}
	for token, e := range st.sessions {
		if e.session.AccountID == accountID {
			delete(st.sessions, token)
		}
	}
}

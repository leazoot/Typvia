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

package handler_test

import (
	"crypto/ed25519"
	"net/http"
	"testing"
	"time"
)

func TestCreateAccountRegistersRootDevice(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")

	createAccount(t, e, d)

	// The registered device can immediately authenticate.
	token := authenticate(t, e, d)
	resp := getPath(t, e.srv, "/v1/devices", token)
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("device directory after account creation: want 200, got %d", resp.StatusCode)
	}
}

func TestCreateAccountRejectsBadRootSignature(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")

	body := d.rootBody(t)
	sig := body["signature"].([]byte)
	sig[0] ^= 0x01
	resp := postJSON(t, e.srv, "/v1/accounts", "", map[string]any{"root": body})
	wantError(t, resp, http.StatusBadRequest, "MALFORMED")
}

func TestCreateAccountRejectsMalformedFields(t *testing.T) {
	t.Parallel()
	e := newEnv(t)

	for name, mutate := range map[string]func(map[string]any){
		"empty device_id": func(b map[string]any) { b["device_id"] = "" },
		"nul in name":     func(b map[string]any) { b["name"] = "bad\x00name" },
		"short ed25519":   func(b map[string]any) { b["ed25519_pub"] = []byte("short") },
		"zero created_at": func(b map[string]any) { b["created_at"] = 0 },
	} {
		d := newDeviceKeys(t, "FAKE_DEV_ROOT")
		body := d.rootBody(t)
		mutate(body)
		resp := postJSON(t, e.srv, "/v1/accounts", "", map[string]any{"root": body})
		if resp.StatusCode != http.StatusBadRequest {
			t.Errorf("%s: want 400, got %d", name, resp.StatusCode)
		}
		resp.Body.Close()
	}
}

func TestCreateAccountRejectsAlreadyRegisteredDevice(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)

	resp := postJSON(t, e.srv, "/v1/accounts", "", map[string]any{"root": d.rootBody(t)})
	wantError(t, resp, http.StatusBadRequest, "MALFORMED")
}

func TestAuthChallengeRejectsUnknownDevice(t *testing.T) {
	t.Parallel()
	e := newEnv(t)

	resp := postJSON(t, e.srv, "/v1/auth/challenge", "", map[string]any{"device_id": "FAKE_UNKNOWN"})
	wantError(t, resp, http.StatusNotFound, "NOT_FOUND")
}

func TestAuthChallengeIsOneTime(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)

	challenge := requestChallenge(t, e, d)
	sig := ed25519.Sign(d.priv, authSignedBytes(challenge, d.id))
	resp := postJSON(t, e.srv, "/v1/auth/session", "", map[string]any{"device_id": d.id, "sig": sig})
	resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("first session exchange: want 200, got %d", resp.StatusCode)
	}

	// The same challenge is burned: replaying the exchange must fail.
	resp = postJSON(t, e.srv, "/v1/auth/session", "", map[string]any{"device_id": d.id, "sig": sig})
	wantError(t, resp, http.StatusUnauthorized, "AUTH_CHALLENGE_EXPIRED")
}

func TestAuthChallengeExpiresAfterTTL(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)

	challenge := requestChallenge(t, e, d)
	e.clock.Advance(61 * time.Second)
	sig := ed25519.Sign(d.priv, authSignedBytes(challenge, d.id))
	resp := postJSON(t, e.srv, "/v1/auth/session", "", map[string]any{"device_id": d.id, "sig": sig})
	wantError(t, resp, http.StatusUnauthorized, "AUTH_CHALLENGE_EXPIRED")
}

func TestAuthSessionRejectsBadSignature(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)

	requestChallenge(t, e, d)
	// Signature over the wrong message (fresh random bytes as challenge).
	sig := ed25519.Sign(d.priv, authSignedBytes(make([]byte, 32), d.id))
	resp := postJSON(t, e.srv, "/v1/auth/session", "", map[string]any{"device_id": d.id, "sig": sig})
	wantError(t, resp, http.StatusBadRequest, "MALFORMED")
}

func TestSessionTokenExpiresAfterTTL(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)

	e.clock.Advance(16 * time.Minute)
	resp := getPath(t, e.srv, "/v1/devices", token)
	wantError(t, resp, http.StatusUnauthorized, "SESSION_EXPIRED")
}

func TestAuthedEndpointRejectsMissingAndUnknownToken(t *testing.T) {
	t.Parallel()
	e := newEnv(t)

	resp := getPath(t, e.srv, "/v1/devices", "")
	wantError(t, resp, http.StatusUnauthorized, "SESSION_EXPIRED")
	resp = getPath(t, e.srv, "/v1/devices", "FAKE_UNKNOWN_TOKEN")
	wantError(t, resp, http.StatusUnauthorized, "SESSION_EXPIRED")
}

func TestAuthChallengeIsRateLimited(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)

	var limited bool
	for i := 0; i < 12; i++ {
		resp := postJSON(t, e.srv, "/v1/auth/challenge", "", map[string]any{"device_id": d.id})
		if resp.StatusCode == http.StatusTooManyRequests {
			if code := decodeErrorBody(t, resp); code != "RATE_LIMITED" {
				t.Fatalf("want RATE_LIMITED, got %q", code)
			}
			limited = true
			resp.Body.Close()
			break
		}
		resp.Body.Close()
	}
	if !limited {
		t.Fatal("challenge burst was never rate limited")
	}
}

func TestProtocolHeaderIsCheckedOnV1Endpoints(t *testing.T) {
	t.Parallel()
	e := newEnv(t)

	req, err := http.NewRequest(http.MethodPost, e.srv.URL+"/v1/auth/challenge", nil)
	if err != nil {
		t.Fatalf("build request: %v", err)
	}
	req.Header.Set("X-Typvia-Protocol", "99")
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("do request: %v", err)
	}
	wantError(t, resp, http.StatusUpgradeRequired, "PROTOCOL_UNSUPPORTED")
}

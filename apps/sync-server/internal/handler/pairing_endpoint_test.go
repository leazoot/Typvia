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
	"bytes"
	"crypto/ed25519"
	"encoding/json"
	"net/http"
	"testing"
	"time"
)

func beginPairing(t *testing.T, e *env, accountID string) string {
	t.Helper()
	resp := postJSON(t, e.srv, "/v1/pair/begin", "", map[string]any{"account_id": accountID})
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("pair begin: want 200, got %d", resp.StatusCode)
	}
	var body struct {
		SessionID string `json:"session_id"`
	}
	decodeBody(t, resp, &body)
	if body.SessionID == "" {
		t.Fatal("pair begin: empty session id")
	}
	return body.SessionID
}

type claimResponse struct {
	Status  string `json:"status"`
	Payload []byte `json:"payload"`
}

func claimPairing(t *testing.T, e *env, sessionID string) (int, claimResponse) {
	t.Helper()
	resp := postJSON(t, e.srv, "/v1/pair/"+sessionID+"/claim", "", nil)
	defer resp.Body.Close()
	var body claimResponse
	if resp.StatusCode == http.StatusOK {
		decodeBody(t, resp, &body)
	}
	return resp.StatusCode, body
}

// certSignedBytes rebuilds the device-certificate message test-side.
func certSignedBytes(deviceID string, edPub, xPub []byte, name, platform string, createdAt, issuedAt int64, issuerID string) []byte {
	var b bytes.Buffer
	b.WriteString("typvia.devcert.v1")
	b.WriteString(deviceID)
	b.WriteByte(0)
	b.Write(edPub)
	b.Write(xPub)
	b.WriteString(name)
	b.WriteByte(0)
	b.WriteString(platform)
	b.WriteByte(0)
	writeLE64(&b, uint64(createdAt))
	writeLE64(&b, uint64(issuedAt))
	b.WriteString(issuerID)
	b.WriteByte(0)
	return b.Bytes()
}

// certBody builds the offer certificate for subject, signed by issuer.
func certBody(issuer, subject *device) map[string]any {
	createdAt := int64(1_700_000_000_000)
	issuedAt := int64(1_700_000_050_000)
	signed := certSignedBytes(subject.id, subject.pub, subject.x25519Pub,
		"FAKE_NEW_DEVICE_NAME", "ios", createdAt, issuedAt, issuer.id)
	return map[string]any{
		"device_id":        subject.id,
		"ed25519_pub":      subject.pub,
		"x25519_pub":       subject.x25519Pub,
		"name":             "FAKE_NEW_DEVICE_NAME",
		"platform":         "ios",
		"created_at":       createdAt,
		"issued_at":        issuedAt,
		"issuer_device_id": issuer.id,
		"signature":        ed25519.Sign(issuer.priv, signed),
	}
}

func offerBody(cert map[string]any) map[string]any {
	return map[string]any{
		"certificate":      cert,
		"sealed_bundle":    []byte("FAKE_SEALED_KEY_BUNDLE"),
		"bundle_signature": bytes.Repeat([]byte{0x42}, 64),
	}
}

func TestPairingRelayRoundTripRegistersTheDevice(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)

	sessionID := beginPairing(t, e, d.accountID)

	// Claim before the offer: pending, session not consumed.
	status, body := claimPairing(t, e, sessionID)
	if status != http.StatusOK || body.Status != "pending" {
		t.Fatalf("claim before offer: want pending, got %d %+v", status, body)
	}

	newDev := newDeviceKeys(t, "FAKE_DEV_NEW")
	resp := postJSON(t, e.srv, "/v1/pair/"+sessionID+"/offer", token, offerBody(certBody(d, newDev)))
	resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("offer: want 200, got %d", resp.StatusCode)
	}

	// The claim payload composes root statement, chain, and sealed bundle.
	status, body = claimPairing(t, e, sessionID)
	if status != http.StatusOK || body.Status != "ready" {
		t.Fatalf("claim: want ready, got %d %+v", status, body)
	}
	var payload struct {
		RootStatement   json.RawMessage   `json:"root_statement"`
		CertChain       []json.RawMessage `json:"cert_chain"`
		SealedBundle    []byte            `json:"sealed_bundle"`
		BundleSignature []byte            `json:"bundle_signature"`
	}
	if err := json.Unmarshal(body.Payload, &payload); err != nil {
		t.Fatalf("claim payload must be the composed JSON: %v", err)
	}
	if len(payload.RootStatement) == 0 || len(payload.CertChain) != 1 {
		t.Fatalf("unexpected claim payload: %+v", payload)
	}
	if string(payload.SealedBundle) != "FAKE_SEALED_KEY_BUNDLE" || len(payload.BundleSignature) != 64 {
		t.Fatalf("sealed bundle must relay opaquely: %+v", payload)
	}

	// Claim is single-use: the session is burned.
	status, _ = claimPairing(t, e, sessionID)
	if status != http.StatusNotFound {
		t.Fatalf("second claim: want 404, got %d", status)
	}

	// The registration side effect: the new device is in the directory
	// with its one-link chain.
	resp = getPath(t, e.srv, "/v1/devices", token)
	defer resp.Body.Close()
	var directory struct {
		Devices []struct {
			ID        string            `json:"id"`
			CertChain []json.RawMessage `json:"cert_chain"`
		} `json:"devices"`
	}
	decodeBody(t, resp, &directory)
	found := false
	for _, dev := range directory.Devices {
		if dev.ID == newDev.id {
			found = true
			if len(dev.CertChain) != 1 {
				t.Fatalf("registered chain must be issuer chain + leaf, got %d links", len(dev.CertChain))
			}
		}
	}
	if !found {
		t.Fatal("offer must register the new device in the directory")
	}
}

func TestPairingOfferIsSingleShotAndAccountBound(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)

	other := newDeviceKeys(t, "FAKE_DEV_OTHER_ACCOUNT")
	createAccount(t, e, other)
	otherToken := authenticate(t, e, other)

	sessionID := beginPairing(t, e, d.accountID)

	// A device of another account cannot post into this session.
	resp := postJSON(t, e.srv, "/v1/pair/"+sessionID+"/offer", otherToken,
		offerBody(certBody(other, newDeviceKeys(t, "FAKE_DEV_FOREIGN"))))
	wantError(t, resp, http.StatusNotFound, "NOT_FOUND")

	resp = postJSON(t, e.srv, "/v1/pair/"+sessionID+"/offer", token,
		offerBody(certBody(d, newDeviceKeys(t, "FAKE_DEV_NEW_1"))))
	resp.Body.Close()

	resp = postJSON(t, e.srv, "/v1/pair/"+sessionID+"/offer", token,
		offerBody(certBody(d, newDeviceKeys(t, "FAKE_DEV_NEW_2"))))
	wantError(t, resp, http.StatusNotFound, "NOT_FOUND")
}

func TestPairingSessionExpiresAfterTTL(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)

	sessionID := beginPairing(t, e, d.accountID)
	e.clock.Advance(11 * time.Minute)

	resp := postJSON(t, e.srv, "/v1/pair/"+sessionID+"/offer", token,
		offerBody(certBody(d, newDeviceKeys(t, "FAKE_DEV_NEW"))))
	wantError(t, resp, http.StatusNotFound, "NOT_FOUND")

	status, _ := claimPairing(t, e, sessionID)
	if status != http.StatusNotFound {
		t.Fatalf("claim of expired session: want 404, got %d", status)
	}
}

func TestPairingBeginRejectsUnknownAccount(t *testing.T) {
	t.Parallel()
	e := newEnv(t)

	resp := postJSON(t, e.srv, "/v1/pair/begin", "", map[string]any{"account_id": "FAKE_UNKNOWN"})
	wantError(t, resp, http.StatusNotFound, "NOT_FOUND")
}

func TestPairingOfferRejectsABadCertificateSignature(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)
	sessionID := beginPairing(t, e, d.accountID)

	cert := certBody(d, newDeviceKeys(t, "FAKE_DEV_NEW"))
	sig := cert["signature"].([]byte)
	sig[0] ^= 0x01
	resp := postJSON(t, e.srv, "/v1/pair/"+sessionID+"/offer", token, offerBody(cert))
	wantError(t, resp, http.StatusBadRequest, "MALFORMED")

	// The rejected offer registered nothing and left the session usable.
	status, body := claimPairing(t, e, sessionID)
	if status != http.StatusOK || body.Status != "pending" {
		t.Fatalf("session must stay pending after a rejected offer, got %d %+v", status, body)
	}
}

func TestPairingOfferRejectsAForeignIssuer(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)
	sessionID := beginPairing(t, e, d.accountID)

	// The certificate claims another issuer than the authenticated device.
	impostor := newDeviceKeys(t, "FAKE_DEV_IMPOSTOR")
	cert := certBody(impostor, newDeviceKeys(t, "FAKE_DEV_NEW"))
	resp := postJSON(t, e.srv, "/v1/pair/"+sessionID+"/offer", token, offerBody(cert))
	wantError(t, resp, http.StatusBadRequest, "MALFORMED")
}

func TestPairingOfferRejectsADuplicateDeviceID(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)
	sessionID := beginPairing(t, e, d.accountID)

	// Re-registering the root device id itself must fail whole.
	self := &device{id: d.id, pub: d.pub, priv: d.priv, x25519Pub: d.x25519Pub}
	resp := postJSON(t, e.srv, "/v1/pair/"+sessionID+"/offer", token, offerBody(certBody(d, self)))
	wantError(t, resp, http.StatusNotFound, "NOT_FOUND")

	// The refused offer rolled back: the session still accepts one.
	status, body := claimPairing(t, e, sessionID)
	if status != http.StatusOK || body.Status != "pending" {
		t.Fatalf("session must stay pending after the rollback, got %d %+v", status, body)
	}
}

func TestPairingOfferFromARevokedDeviceIsRejected(t *testing.T) {
	t.Parallel()
	e, st := envWithStore(t)
	root, second := twoDeviceFixture(t, e, st)
	rootToken := authenticate(t, e, root)
	secondToken := authenticate(t, e, second)

	sessionID := beginPairing(t, e, root.accountID)

	// Root revokes the second device; its already-issued session token
	// must no longer admit an offer (the revocation execution surface).
	revokedAt := int64(1_700_000_200_000)
	resp := postJSON(t, e.srv, "/v1/devices/"+second.id+"/revoke", rootToken, map[string]any{
		"revoked_at": revokedAt,
		"signature":  root.revokeSig(second.id, revokedAt),
	})
	resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("revoke: want 200, got %d", resp.StatusCode)
	}

	resp = postJSON(t, e.srv, "/v1/pair/"+sessionID+"/offer", secondToken,
		offerBody(certBody(second, newDeviceKeys(t, "FAKE_DEV_NEW"))))
	// The dropped session surfaces as SESSION_EXPIRED, or — were a token
	// still live — the active-device check answers DEVICE_REVOKED; both
	// deny the revoked admitter.
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusForbidden && resp.StatusCode != http.StatusUnauthorized {
		t.Fatalf("offer from revoked device: want 401/403, got %d", resp.StatusCode)
	}
}

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

package handler

import (
	"net/http"

	"typvia.dev/sync-server/internal/service"
)

// beginPairing handles POST /v1/pair/begin (step 1; the caller holds
// account_id from the pairing code, no device session exists yet).
func (a *api) beginPairing(w http.ResponseWriter, r *http.Request) {
	var body struct {
		AccountID string `json:"account_id"`
	}
	if err := decodeJSON(w, r, maxSmallBody, &body); err != nil {
		writeError(w, err)
		return
	}
	out, err := a.svc.BeginPairing(r.Context(), body.AccountID)
	if err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"session_id": out.SessionID,
		"expires_in": out.ExpiresIn,
	})
}

// pairingCertBody is the JSON form of the new device's certificate in an
// offer request ([]byte fields are base64).
type pairingCertBody struct {
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

func (b pairingCertBody) toInput() service.PairingCertificateInput {
	return service.PairingCertificateInput{
		DeviceID:       b.DeviceID,
		Ed25519Pub:     b.Ed25519Pub,
		X25519Pub:      b.X25519Pub,
		Name:           b.Name,
		Platform:       b.Platform,
		CreatedAt:      b.CreatedAt,
		IssuedAt:       b.IssuedAt,
		IssuerDeviceID: b.IssuerDeviceID,
		Signature:      b.Signature,
	}
}

// pairingOffer handles POST /v1/pair/{session}/offer (steps 6-7: the
// admitting device posts the new device's certificate and the sealed key
// bundle; the server verifies the certificate and registers the device in
// the same transaction; single-shot).
func (a *api) pairingOffer(w http.ResponseWriter, r *http.Request, sess service.Session) {
	var body struct {
		Certificate     pairingCertBody `json:"certificate"`
		SealedBundle    []byte          `json:"sealed_bundle"`
		BundleSignature []byte          `json:"bundle_signature"`
	}
	if err := decodeJSON(w, r, maxSmallBody, &body); err != nil {
		writeError(w, err)
		return
	}
	err := a.svc.PostPairingOffer(r.Context(), sess, r.PathValue("session"),
		body.Certificate.toInput(), body.SealedBundle, body.BundleSignature)
	if err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"status": "offered"})
}

// pairingClaim handles POST /v1/pair/{session}/claim (step 7: the new
// device takes the relay payload, single-use; pending until offered).
func (a *api) pairingClaim(w http.ResponseWriter, r *http.Request) {
	out, err := a.svc.ClaimPairing(r.Context(), r.PathValue("session"))
	if err != nil {
		writeError(w, err)
		return
	}
	if out.Pending {
		writeJSON(w, http.StatusOK, map[string]any{"status": "pending"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"status": "ready", "payload": out.Payload})
}

// addKeyUpdate handles POST /v1/keys/updates: a sealed device-to-device
// key message.
func (a *api) addKeyUpdate(w http.ResponseWriter, r *http.Request, sess service.Session) {
	var body struct {
		TargetDeviceID string `json:"target_device_id"`
		Payload        []byte `json:"payload"`
	}
	if err := decodeJSON(w, r, maxSmallBody, &body); err != nil {
		writeError(w, err)
		return
	}
	seq, err := a.svc.AddKeyUpdate(r.Context(), sess, body.TargetDeviceID, body.Payload)
	if err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]int64{"seq": seq})
}

// listKeyUpdates handles GET /v1/keys/updates?since=: a device pulls
// only its own messages.
func (a *api) listKeyUpdates(w http.ResponseWriter, r *http.Request, sess service.Session) {
	since, err := queryInt(r, "since", 0)
	if err != nil {
		writeError(w, err)
		return
	}
	updates, svcErr := a.svc.ListKeyUpdates(r.Context(), sess, since)
	if svcErr != nil {
		writeError(w, svcErr)
		return
	}
	if updates == nil {
		updates = []service.KeyUpdateOut{}
	}
	writeJSON(w, http.StatusOK, map[string]any{"updates": updates})
}

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

// putRecovery handles PUT /v1/recovery: store the recovery blob and the
// rootproof public key under a device session.
func (a *api) putRecovery(w http.ResponseWriter, r *http.Request, sess service.Session) {
	var body struct {
		Blob         []byte `json:"blob"`
		RootproofPub []byte `json:"rootproof_pub"`
	}
	if err := decodeJSON(w, r, maxSmallBody, &body); err != nil {
		writeError(w, err)
		return
	}
	if err := a.svc.PutRecoveryBlob(r.Context(), sess, body.Blob, body.RootproofPub); err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"status": "stored"})
}

// getRecovery handles GET /v1/recovery?account_id=: an anonymous fetch
// under escalating per ip|account backoff — the recovery code's 128-bit
// entropy is the main defense, the backoff blunts online guessing.
func (a *api) getRecovery(w http.ResponseWriter, r *http.Request) {
	accountID := r.URL.Query().Get("account_id")
	if !a.recoveryBackoff.Allow(clientIP(r) + "|" + accountID) {
		writeError(w, service.RateLimitedErr("too many recovery requests; retry later"))
		return
	}
	blob, err := a.svc.GetRecoveryBlob(r.Context(), accountID)
	if err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string][]byte{"blob": blob})
}

// reRoot handles POST /v1/root. Two phases share the endpoint: a body
// without proof_sig requests a one-time root challenge; a body with
// proof_sig and new_root executes the root replacement.
func (a *api) reRoot(w http.ResponseWriter, r *http.Request) {
	var body struct {
		AccountID string          `json:"account_id"`
		ProofSig  []byte          `json:"proof_sig"`
		NewRoot   *rootDeviceBody `json:"new_root"`
	}
	if err := decodeJSON(w, r, maxSmallBody, &body); err != nil {
		writeError(w, err)
		return
	}
	if len(body.ProofSig) == 0 && body.NewRoot == nil {
		out, err := a.svc.IssueRootChallenge(r.Context(), body.AccountID)
		if err != nil {
			writeError(w, err)
			return
		}
		writeJSON(w, http.StatusOK, map[string]any{
			"challenge":  out.Challenge,
			"expires_in": out.ExpiresIn,
		})
		return
	}
	if body.NewRoot == nil {
		writeError(w, service.MalformedErr("new_root is required alongside proof_sig"))
		return
	}
	if err := a.svc.ReRoot(r.Context(), body.AccountID, body.ProofSig, body.NewRoot.toInput()); err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"status": "re-rooted"})
}

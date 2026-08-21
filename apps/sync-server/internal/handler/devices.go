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

// deviceDirectory handles GET /v1/devices: the device directory with
// certificate chains and revocation statements.
func (a *api) deviceDirectory(w http.ResponseWriter, r *http.Request, sess service.Session) {
	out, err := a.svc.DeviceDirectory(r.Context(), sess)
	if err != nil {
		writeError(w, err)
		return
	}
	if out.Devices == nil {
		out.Devices = []service.DeviceOut{}
	}
	if out.Revocations == nil {
		out.Revocations = []service.RevocationOut{}
	}
	writeJSON(w, http.StatusOK, out)
}

// revokeDevice handles POST /v1/devices/{id}/revoke.
func (a *api) revokeDevice(w http.ResponseWriter, r *http.Request, sess service.Session) {
	var body struct {
		RevokedAt int64  `json:"revoked_at"`
		Signature []byte `json:"signature"`
	}
	if err := decodeJSON(w, r, maxSmallBody, &body); err != nil {
		writeError(w, err)
		return
	}
	err := a.svc.RevokeDevice(r.Context(), sess, r.PathValue("id"), body.RevokedAt, body.Signature)
	if err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"status": "revoked"})
}

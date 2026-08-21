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
	"strconv"

	"typvia.dev/sync-server/internal/service"
)

// recordBody is the wire form of one pushed record; []byte fields are
// base64 in JSON.
type recordBody struct {
	ID         string `json:"id"`
	EntityType string `json:"entity_type"`
	EntityID   string `json:"entity_id"`
	Version    int64  `json:"version"`
	Ciphertext []byte `json:"ciphertext"`
	DeletedAt  *int64 `json:"deleted_at"`
	UpdatedAt  int64  `json:"updated_at"`
	DeviceID   string `json:"device_id"`
	KeyID      int64  `json:"key_id"`
	Signature  []byte `json:"signature"`
}

// pushRecords handles POST /v1/records (batch push).
func (a *api) pushRecords(w http.ResponseWriter, r *http.Request, sess service.Session) {
	if !a.recordsLimit.Allow(sess.DeviceID) {
		writeError(w, service.RateLimitedErr("too many record requests; retry later"))
		return
	}
	var body struct {
		Records []recordBody `json:"records"`
	}
	if err := decodeJSON(w, r, maxRecordsBody, &body); err != nil {
		writeError(w, err)
		return
	}
	records := make([]service.RecordIn, len(body.Records))
	for i, rec := range body.Records {
		records[i] = service.RecordIn{
			ID:         rec.ID,
			EntityType: rec.EntityType,
			EntityID:   rec.EntityID,
			Version:    rec.Version,
			Ciphertext: rec.Ciphertext,
			DeletedAt:  rec.DeletedAt,
			UpdatedAt:  rec.UpdatedAt,
			DeviceID:   rec.DeviceID,
			KeyID:      rec.KeyID,
			Signature:  rec.Signature,
		}
	}
	results, err := a.svc.PushRecords(r.Context(), sess, records)
	if err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"results": results})
}

// pullRecords handles GET /v1/records?since= (incremental pull).
func (a *api) pullRecords(w http.ResponseWriter, r *http.Request, sess service.Session) {
	if !a.recordsLimit.Allow(sess.DeviceID) {
		writeError(w, service.RateLimitedErr("too many record requests; retry later"))
		return
	}
	since, err := queryInt(r, "since", 0)
	if err != nil {
		writeError(w, err)
		return
	}
	limit, err := queryInt(r, "limit", service.PullPageLimit)
	if err != nil {
		writeError(w, err)
		return
	}
	out, svcErr := a.svc.PullRecords(r.Context(), sess, since, int(limit))
	if svcErr != nil {
		writeError(w, svcErr)
		return
	}
	if out.Records == nil {
		out.Records = []service.RecordOut{}
	}
	writeJSON(w, http.StatusOK, out)
}

// queryInt parses an optional non-negative integer query parameter.
func queryInt(r *http.Request, name string, def int64) (int64, error) {
	raw := r.URL.Query().Get(name)
	if raw == "" {
		return def, nil
	}
	v, err := strconv.ParseInt(raw, 10, 64)
	if err != nil || v < 0 {
		return 0, service.MalformedErr(name + " must be a non-negative integer")
	}
	return v, nil
}

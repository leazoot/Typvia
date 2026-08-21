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
	"encoding/json"
	"errors"
	"log/slog"
	"net/http"

	"typvia.dev/sync-server/internal/service"
)

// errorBody is the stable error envelope shared by all endpoints: a stable
// string code plus a human-readable message that never echoes request
// content.
type errorBody struct {
	Error errorDetail `json:"error"`
}

type errorDetail struct {
	Code    string `json:"code"`
	Message string `json:"message"`
	// Conflicts carries current entity head versions on VERSION_CONFLICT;
	// omitted for every other code.
	Conflicts []service.ConflictHead `json:"conflicts,omitempty"`
}

// codeStatus pins codes whose HTTP status deviates from the kind default.
var codeStatus = map[string]int{
	service.CodeProtocolUnsupported:  http.StatusUpgradeRequired,
	service.CodeNotFound:             http.StatusNotFound,
	service.CodeAuthChallengeExpired: http.StatusUnauthorized,
	service.CodeSessionExpired:       http.StatusUnauthorized,
	service.CodeDeviceRevoked:        http.StatusForbidden,
	service.CodePayloadTooLarge:      http.StatusRequestEntityTooLarge,
	service.CodeRateLimited:          http.StatusTooManyRequests,
}

func writeJSON(w http.ResponseWriter, status int, body any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if err := json.NewEncoder(w).Encode(body); err != nil {
		// Headers are already written; the failed body cannot be replaced.
		slog.Error("write response body", "error", err)
	}
}

// writeError maps a service error to the HTTP error envelope. Non-service
// errors are treated as system errors and their details stay server-side.
func writeError(w http.ResponseWriter, err error) {
	var svcErr *service.Error
	if !errors.As(err, &svcErr) {
		slog.Error("unclassified handler error", "error", err)
		writeJSON(w, http.StatusInternalServerError, errorBody{
			Error: errorDetail{Code: service.CodeInternal, Message: "internal server error"},
		})
		return
	}
	status, ok := codeStatus[svcErr.Code]
	if !ok {
		switch svcErr.Kind {
		case service.KindProtocol:
			status = http.StatusBadRequest
		case service.KindBusiness:
			status = http.StatusConflict
		default:
			status = http.StatusInternalServerError
		}
	}
	if svcErr.Kind == service.KindSystem {
		// System causes are logged, not returned; the log carries no request
		// content, ciphertext, or key material by construction.
		slog.Error("request failed", "code", svcErr.Code, "error", svcErr.Unwrap())
	}
	writeJSON(w, status, errorBody{Error: errorDetail{
		Code:      svcErr.Code,
		Message:   svcErr.Message,
		Conflicts: svcErr.Conflicts,
	}})
}

// decodeJSON reads the request body (capped at maxBytes) into dst, mapping
// oversized bodies to PAYLOAD_TOO_LARGE and undecodable ones to MALFORMED.
// Error messages never echo body content.
func decodeJSON(w http.ResponseWriter, r *http.Request, maxBytes int64, dst any) error {
	r.Body = http.MaxBytesReader(w, r.Body, maxBytes)
	if err := json.NewDecoder(r.Body).Decode(dst); err != nil {
		var tooLarge *http.MaxBytesError
		if errors.As(err, &tooLarge) {
			return service.PayloadTooLargeErr("request body exceeds the size limit")
		}
		return service.MalformedErr("request body is not valid JSON for this endpoint")
	}
	return nil
}

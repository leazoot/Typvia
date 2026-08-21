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

import "fmt"

// ErrorKind is the three-way error taxonomy of the server: protocol errors
// (the client speaks an unsupported or malformed dialect), business errors
// (valid request, rejected by state such as conflicts), and system errors
// (server-side failure). Handlers map Kind + Code to HTTP responses.
type ErrorKind int

const (
	// KindProtocol: request violates the sync protocol contract.
	KindProtocol ErrorKind = iota + 1
	// KindBusiness: request is well-formed but rejected by current state.
	KindBusiness
	// KindSystem: server-side failure; safe to retry later.
	KindSystem
)

// Stable string error codes returned to clients.
const (
	CodeProtocolUnsupported  = "PROTOCOL_UNSUPPORTED"
	CodeAuthChallengeExpired = "AUTH_CHALLENGE_EXPIRED"
	CodeDeviceRevoked        = "DEVICE_REVOKED"
	CodeVersionConflict      = "VERSION_CONFLICT"
	CodePayloadTooLarge      = "PAYLOAD_TOO_LARGE"
	CodeRateLimited          = "RATE_LIMITED"
	CodeSessionExpired       = "SESSION_EXPIRED"
	CodeNotFound             = "NOT_FOUND"
	CodeMalformed            = "MALFORMED"
	// CodeInternal is the generic system-failure code; details stay in
	// server logs, never in the response.
	CodeInternal = "INTERNAL"
)

// ConflictHead reports the current server head version of one entity in a
// VERSION_CONFLICT response (409 carries current heads).
type ConflictHead struct {
	EntityType  string `json:"entity_type"`
	EntityID    string `json:"entity_id"`
	HeadVersion int64  `json:"head_version"`
}

// Error is the service-level error carrying the stable code returned to
// clients. Message must never contain request body content, ciphertext,
// or key material — responses never echo the request.
type Error struct {
	Kind    ErrorKind
	Code    string
	Message string
	// Conflicts carries the current entity heads of a VERSION_CONFLICT
	// response; nil for every other code.
	Conflicts []ConflictHead
	// cause is retained for server-side logging/wrapping only.
	cause error
}

// Error implements the error interface.
func (e *Error) Error() string {
	if e.cause != nil {
		return fmt.Sprintf("%s: %s: %v", e.Code, e.Message, e.cause)
	}
	return fmt.Sprintf("%s: %s", e.Code, e.Message)
}

// Unwrap exposes the underlying cause for errors.Is/As.
func (e *Error) Unwrap() error {
	return e.cause
}

func protocolErr(code, message string) *Error {
	return &Error{Kind: KindProtocol, Code: code, Message: message}
}

func systemErr(cause error) *Error {
	return &Error{Kind: KindSystem, Code: CodeInternal, Message: "internal server error", cause: cause}
}

func businessErr(code, message string) *Error {
	return &Error{Kind: KindBusiness, Code: code, Message: message}
}

// NotFoundErr is the shared business error for unknown routes and missing
// resources.
func NotFoundErr(message string) *Error {
	return &Error{Kind: KindBusiness, Code: CodeNotFound, Message: message}
}

// MalformedErr is the shared protocol error for requests that fail input
// validation or admission signature checks. The message describes the rule,
// never the submitted value.
func MalformedErr(message string) *Error {
	return &Error{Kind: KindProtocol, Code: CodeMalformed, Message: message}
}

// RateLimitedErr is returned by handlers when a rate limit rejects the
// request before it reaches the business layer.
func RateLimitedErr(message string) *Error {
	return &Error{Kind: KindBusiness, Code: CodeRateLimited, Message: message}
}

// PayloadTooLargeErr rejects requests exceeding the protocol size limits,
// whether caught at the transport cap (handler) or on decoded sizes
// (service).
func PayloadTooLargeErr(message string) *Error {
	return &Error{Kind: KindProtocol, Code: CodePayloadTooLarge, Message: message}
}

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
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"

	"typvia.dev/sync-server/internal/handler"
	"typvia.dev/sync-server/internal/service"
	"typvia.dev/sync-server/internal/store"
)

// newTestServer wires the real three layers over a temporary database.
func newTestServer(t *testing.T) *httptest.Server {
	t.Helper()
	st, err := store.Open(filepath.Join(t.TempDir(), "test.db"))
	if err != nil {
		t.Fatalf("open store: %v", err)
	}
	t.Cleanup(func() {
		if err := st.Close(); err != nil {
			t.Errorf("close store: %v", err)
		}
	})
	if err := st.Migrate(context.Background()); err != nil {
		t.Fatalf("migrate: %v", err)
	}
	srv := httptest.NewServer(handler.New(service.New(st)))
	t.Cleanup(srv.Close)
	return srv
}

// decodeErrorBody parses the shared error envelope and returns the code.
func decodeErrorBody(t *testing.T, resp *http.Response) string {
	t.Helper()
	var body struct {
		Error struct {
			Code    string `json:"code"`
			Message string `json:"message"`
		} `json:"error"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&body); err != nil {
		t.Fatalf("decode error body: %v", err)
	}
	if body.Error.Message == "" {
		t.Error("error message must not be empty")
	}
	return body.Error.Code
}

func TestHealthzReportsOK(t *testing.T) {
	t.Parallel()
	srv := newTestServer(t)

	resp, err := http.Get(srv.URL + "/healthz")
	if err != nil {
		t.Fatalf("get healthz: %v", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("want 200, got %d", resp.StatusCode)
	}
	var body map[string]string
	if err := json.NewDecoder(resp.Body).Decode(&body); err != nil {
		t.Fatalf("decode body: %v", err)
	}
	if body["status"] != "ok" {
		t.Fatalf("want status ok, got %+v", body)
	}
}

func TestHandshakeReturnsProtocolWindow(t *testing.T) {
	t.Parallel()
	srv := newTestServer(t)

	resp, err := http.Get(srv.URL + "/v1/handshake")
	if err != nil {
		t.Fatalf("get handshake: %v", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("want 200, got %d", resp.StatusCode)
	}
	var body struct {
		ProtocolMin   int    `json:"protocol_min"`
		ProtocolMax   int    `json:"protocol_max"`
		ServerVersion string `json:"server_version"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&body); err != nil {
		t.Fatalf("decode body: %v", err)
	}
	if body.ProtocolMin != 1 || body.ProtocolMax != 1 || body.ServerVersion == "" {
		t.Fatalf("unexpected handshake body: %+v", body)
	}
}

func TestHandshakeAcceptsSupportedProtocolHeader(t *testing.T) {
	t.Parallel()
	srv := newTestServer(t)

	req, err := http.NewRequest(http.MethodGet, srv.URL+"/v1/handshake", nil)
	if err != nil {
		t.Fatalf("build request: %v", err)
	}
	req.Header.Set("X-Typvia-Protocol", "1")
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("do request: %v", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("want 200 for supported protocol, got %d", resp.StatusCode)
	}
}

func TestHandshakeRejectsUnsupportedProtocolHeader(t *testing.T) {
	t.Parallel()
	srv := newTestServer(t)

	// An unsupported version header → 426 with the stable
	// PROTOCOL_UNSUPPORTED code; non-numeric headers are equally unsupported.
	for _, headerValue := range []string{"2", "0", "-1", "abc"} {
		req, err := http.NewRequest(http.MethodGet, srv.URL+"/v1/handshake", nil)
		if err != nil {
			t.Fatalf("build request: %v", err)
		}
		req.Header.Set("X-Typvia-Protocol", headerValue)
		resp, err := http.DefaultClient.Do(req)
		if err != nil {
			t.Fatalf("do request: %v", err)
		}
		if resp.StatusCode != http.StatusUpgradeRequired {
			t.Errorf("header %q: want 426, got %d", headerValue, resp.StatusCode)
		}
		if code := decodeErrorBody(t, resp); code != "PROTOCOL_UNSUPPORTED" {
			t.Errorf("header %q: want code PROTOCOL_UNSUPPORTED, got %q", headerValue, code)
		}
		resp.Body.Close()
	}
}

func TestUnknownRouteReturnsJSONNotFound(t *testing.T) {
	t.Parallel()
	srv := newTestServer(t)

	resp, err := http.Get(srv.URL + "/v1/nope")
	if err != nil {
		t.Fatalf("get unknown route: %v", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusNotFound {
		t.Fatalf("want 404, got %d", resp.StatusCode)
	}
	if code := decodeErrorBody(t, resp); code != "NOT_FOUND" {
		t.Fatalf("want code NOT_FOUND, got %q", code)
	}
}

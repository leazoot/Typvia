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
	"net/http"
	"testing"
)

type pushResponse struct {
	Results []struct {
		ID        string `json:"id"`
		ServerSeq int64  `json:"server_seq"`
		Duplicate bool   `json:"duplicate"`
	} `json:"results"`
}

type pullResponse struct {
	Records []struct {
		ID         string `json:"id"`
		EntityID   string `json:"entity_id"`
		Version    int64  `json:"version"`
		Ciphertext []byte `json:"ciphertext"`
		DeletedAt  *int64 `json:"deleted_at"`
		ServerSeq  int64  `json:"server_seq"`
	} `json:"records"`
	HasMore bool `json:"has_more"`
}

func pushRecords(t *testing.T, e *env, token string, records ...map[string]any) *http.Response {
	t.Helper()
	return postJSON(t, e.srv, "/v1/records", token, map[string]any{"records": records})
}

func TestPushAndPullRoundTrip(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)

	ct := []byte("FAKE_CIPHERTEXT_E1_V1")
	resp := pushRecords(t, e, token,
		d.signedRecord("FAKE_REC_1", "snippet", "FAKE_E1", 1, ct, nil),
		d.signedRecord("FAKE_REC_2", "folder", "FAKE_F1", 1, []byte("FAKE_CIPHERTEXT_F1"), nil),
	)
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("push: want 200, got %d", resp.StatusCode)
	}
	var pushed pushResponse
	decodeBody(t, resp, &pushed)
	if len(pushed.Results) != 2 || pushed.Results[0].ServerSeq != 1 || pushed.Results[1].ServerSeq != 2 {
		t.Fatalf("unexpected push results: %+v", pushed.Results)
	}

	pull := getPath(t, e.srv, "/v1/records?since=0", token)
	defer pull.Body.Close()
	if pull.StatusCode != http.StatusOK {
		t.Fatalf("pull: want 200, got %d", pull.StatusCode)
	}
	var pulled pullResponse
	decodeBody(t, pull, &pulled)
	if len(pulled.Records) != 2 || pulled.HasMore {
		t.Fatalf("unexpected pull page: %d records has_more=%v", len(pulled.Records), pulled.HasMore)
	}
	if pulled.Records[0].ServerSeq != 1 || !bytes.Equal(pulled.Records[0].Ciphertext, ct) {
		t.Fatalf("first pulled record mismatch: %+v", pulled.Records[0])
	}
}

func TestPushIsIdempotentOnRecordID(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)

	rec := d.signedRecord("FAKE_REC_1", "snippet", "FAKE_E1", 1, []byte("FAKE_CIPHERTEXT"), nil)
	resp := pushRecords(t, e, token, rec)
	resp.Body.Close()

	resp = pushRecords(t, e, token, rec)
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("idempotent re-push: want 200, got %d", resp.StatusCode)
	}
	var pushed pushResponse
	decodeBody(t, resp, &pushed)
	if !pushed.Results[0].Duplicate || pushed.Results[0].ServerSeq != 1 {
		t.Fatalf("want duplicate at seq 1, got %+v", pushed.Results[0])
	}
}

func TestPushRejectsStaleVersionWithCurrentHeads(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)

	resp := pushRecords(t, e, token,
		d.signedRecord("FAKE_REC_1", "snippet", "FAKE_E1", 1, []byte("FAKE_CT_1"), nil))
	resp.Body.Close()

	// A replayed version 1 under a new record id is a conflict.
	resp = pushRecords(t, e, token,
		d.signedRecord("FAKE_REC_STALE", "snippet", "FAKE_E1", 1, []byte("FAKE_CT_STALE"), nil))
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusConflict {
		t.Fatalf("stale push: want 409, got %d", resp.StatusCode)
	}
	var body struct {
		Error struct {
			Code      string `json:"code"`
			Conflicts []struct {
				EntityType  string `json:"entity_type"`
				EntityID    string `json:"entity_id"`
				HeadVersion int64  `json:"head_version"`
			} `json:"conflicts"`
		} `json:"error"`
	}
	decodeBody(t, resp, &body)
	if body.Error.Code != "VERSION_CONFLICT" {
		t.Fatalf("want VERSION_CONFLICT, got %q", body.Error.Code)
	}
	if len(body.Error.Conflicts) != 1 || body.Error.Conflicts[0].EntityID != "FAKE_E1" || body.Error.Conflicts[0].HeadVersion != 1 {
		t.Fatalf("want conflict head FAKE_E1@1, got %+v", body.Error.Conflicts)
	}
}

func TestPushAcceptsTombstoneAndRejectsTombstoneWithCiphertext(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)

	resp := pushRecords(t, e, token,
		d.signedRecord("FAKE_REC_1", "snippet", "FAKE_E1", 1, []byte("FAKE_CT"), nil))
	resp.Body.Close()

	deletedAt := int64(1_700_000_200_000)
	resp = pushRecords(t, e, token,
		d.signedRecord("FAKE_REC_2", "snippet", "FAKE_E1", 2, nil, &deletedAt))
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("tombstone push: want 200, got %d", resp.StatusCode)
	}
	resp.Body.Close()

	// Tombstone carrying ciphertext violates the storage invariant.
	bad := d.signedRecord("FAKE_REC_3", "snippet", "FAKE_E1", 3, nil, &deletedAt)
	bad["ciphertext"] = []byte("FAKE_CT_ON_TOMBSTONE")
	resp = pushRecords(t, e, token, bad)
	wantError(t, resp, http.StatusBadRequest, "MALFORMED")
}

func TestPushRejectsBadSignatureAndForeignDevice(t *testing.T) {
	t.Parallel()
	e, st := envWithStore(t)
	root, second := twoDeviceFixture(t, e, st)
	token := authenticate(t, e, root)

	rec := root.signedRecord("FAKE_REC_1", "snippet", "FAKE_E1", 1, []byte("FAKE_CT"), nil)
	rec["signature"] = append([]byte{}, rec["signature"].([]byte)...)
	rec["signature"].([]byte)[0] ^= 0x01
	resp := pushRecords(t, e, token, rec)
	wantError(t, resp, http.StatusBadRequest, "MALFORMED")

	// A record produced (and signed) by another device is rejected: pushes
	// are self-produced only.
	foreign := second.signedRecord("FAKE_REC_2", "snippet", "FAKE_E2", 1, []byte("FAKE_CT"), nil)
	resp = pushRecords(t, e, token, foreign)
	wantError(t, resp, http.StatusBadRequest, "MALFORMED")
}

func TestPushRejectsUnknownEntityType(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)

	rec := d.signedRecord("FAKE_REC_1", "device", "FAKE_E1", 1, []byte("FAKE_CT"), nil)
	resp := pushRecords(t, e, token, rec)
	wantError(t, resp, http.StatusBadRequest, "MALFORMED")
}

func TestPushRejectsOversizedEnvelopeAndBatch(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)

	// Single envelope beyond 256KB.
	big := bytes.Repeat([]byte("A"), 256*1024+1)
	resp := pushRecords(t, e, token,
		d.signedRecord("FAKE_REC_BIG", "snippet", "FAKE_E1", 1, big, nil))
	wantError(t, resp, http.StatusRequestEntityTooLarge, "PAYLOAD_TOO_LARGE")

	// Batch beyond 500 records. Validation rejects on count before
	// signatures are checked, so unsigned stubs suffice.
	records := make([]map[string]any, 501)
	for i := range records {
		records[i] = d.signedRecord("FAKE_REC", "snippet", "FAKE_E1", 1, []byte("FAKE_CT"), nil)
	}
	resp = postJSON(t, e.srv, "/v1/records", token, map[string]any{"records": records})
	wantError(t, resp, http.StatusRequestEntityTooLarge, "PAYLOAD_TOO_LARGE")
}

func TestPullPaginatesWithHasMore(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)

	var records []map[string]any
	for i := 0; i < 3; i++ {
		records = append(records, d.signedRecord(
			"FAKE_REC_"+string(rune('A'+i)), "snippet", "FAKE_E1", int64(i+1), []byte("FAKE_CT"), nil))
	}
	resp := postJSON(t, e.srv, "/v1/records", token, map[string]any{"records": records})
	resp.Body.Close()

	page := getPath(t, e.srv, "/v1/records?since=0&limit=2", token)
	defer page.Body.Close()
	var pulled pullResponse
	decodeBody(t, page, &pulled)
	if len(pulled.Records) != 2 || !pulled.HasMore {
		t.Fatalf("want 2 records has_more, got %d has_more=%v", len(pulled.Records), pulled.HasMore)
	}

	rest := getPath(t, e.srv, "/v1/records?since=2", token)
	defer rest.Body.Close()
	decodeBody(t, rest, &pulled)
	if len(pulled.Records) != 1 || pulled.HasMore || pulled.Records[0].ServerSeq != 3 {
		t.Fatalf("want final record seq 3, got %+v has_more=%v", pulled.Records, pulled.HasMore)
	}
}

func TestPushRequiresSession(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")

	resp := pushRecords(t, e, "",
		d.signedRecord("FAKE_REC_1", "snippet", "FAKE_E1", 1, []byte("FAKE_CT"), nil))
	wantError(t, resp, http.StatusUnauthorized, "SESSION_EXPIRED")
}

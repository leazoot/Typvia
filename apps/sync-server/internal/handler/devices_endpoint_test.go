package handler_test

import (
	"context"
	"encoding/json"
	"net/http"
	"testing"

	"typvia.dev/sync-server/internal/store"
)

type directoryResponse struct {
	RootStatement json.RawMessage `json:"root_statement"`
	Devices       []struct {
		ID         string          `json:"id"`
		Ed25519Pub []byte          `json:"ed25519_pub"`
		CertChain  json.RawMessage `json:"cert_chain"`
		RevokedAt  *int64          `json:"revoked_at"`
	} `json:"devices"`
	Revocations []struct {
		DeviceID  string          `json:"device_id"`
		Statement json.RawMessage `json:"statement"`
		RevokedAt int64           `json:"revoked_at"`
	} `json:"revocations"`
}

func fetchDirectory(t *testing.T, e *env, token string) directoryResponse {
	t.Helper()
	resp := getPath(t, e.srv, "/v1/devices", token)
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("device directory: want 200, got %d", resp.StatusCode)
	}
	var dir directoryResponse
	decodeBody(t, resp, &dir)
	return dir
}

func TestDeviceDirectoryListsRootDevicesAndRevocations(t *testing.T) {
	t.Parallel()
	e, st := envWithStore(t)
	root, _ := twoDeviceFixture(t, e, st)
	token := authenticate(t, e, root)

	dir := fetchDirectory(t, e, token)
	if len(dir.RootStatement) == 0 {
		t.Fatal("directory must carry the root statement")
	}
	var rootDoc struct {
		DeviceID  string `json:"device_id"`
		Signature []byte `json:"signature"`
	}
	if err := json.Unmarshal(dir.RootStatement, &rootDoc); err != nil {
		t.Fatalf("root statement is not the stored JSON doc: %v", err)
	}
	if rootDoc.DeviceID != root.id || len(rootDoc.Signature) != 64 {
		t.Fatalf("unexpected root statement doc: %+v", rootDoc)
	}
	if len(dir.Devices) != 2 {
		t.Fatalf("want 2 devices, got %d", len(dir.Devices))
	}
	if len(dir.Revocations) != 0 {
		t.Fatalf("want no revocations yet, got %d", len(dir.Revocations))
	}
}

func TestRevokeDeviceMarksDeviceAndCutsItsAccess(t *testing.T) {
	t.Parallel()
	e, st := envWithStore(t)
	root, second := twoDeviceFixture(t, e, st)
	rootToken := authenticate(t, e, root)
	secondToken := authenticate(t, e, second)

	revokedAt := int64(1_700_000_300_000)
	resp := postJSON(t, e.srv, "/v1/devices/"+second.id+"/revoke", rootToken, map[string]any{
		"revoked_at": revokedAt,
		"signature":  root.revokeSig(second.id, revokedAt),
	})
	resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("revoke: want 200, got %d", resp.StatusCode)
	}

	dir := fetchDirectory(t, e, rootToken)
	if len(dir.Revocations) != 1 || dir.Revocations[0].DeviceID != second.id {
		t.Fatalf("want one revocation of %s, got %+v", second.id, dir.Revocations)
	}
	var found bool
	for _, d := range dir.Devices {
		if d.ID == second.id {
			found = true
			if d.RevokedAt == nil || *d.RevokedAt != revokedAt {
				t.Fatalf("want device revoked_at %d, got %+v", revokedAt, d.RevokedAt)
			}
		}
	}
	if !found {
		t.Fatal("revoked device must stay in the directory")
	}

	// The revoked device's existing session was invalidated on revocation:
	// its token is gone, so the request reads as an expired session.
	resp = getPath(t, e.srv, "/v1/devices", secondToken)
	wantError(t, resp, http.StatusUnauthorized, "SESSION_EXPIRED")

	// And a new challenge is refused at step one.
	resp = postJSON(t, e.srv, "/v1/auth/challenge", "", map[string]any{"device_id": second.id})
	wantError(t, resp, http.StatusForbidden, "DEVICE_REVOKED")
}

func TestRevokeDeviceRejectsBadSignature(t *testing.T) {
	t.Parallel()
	e, st := envWithStore(t)
	root, second := twoDeviceFixture(t, e, st)
	rootToken := authenticate(t, e, root)

	// Signature by the wrong key (the target's own key, not the issuer's).
	resp := postJSON(t, e.srv, "/v1/devices/"+second.id+"/revoke", rootToken, map[string]any{
		"revoked_at": int64(1),
		"signature":  second.revokeSig(second.id, 1),
	})
	wantError(t, resp, http.StatusBadRequest, "MALFORMED")
}

func TestRevokeDeviceRejectsRepeatAndUnknownTarget(t *testing.T) {
	t.Parallel()
	e, st := envWithStore(t)
	root, second := twoDeviceFixture(t, e, st)
	rootToken := authenticate(t, e, root)

	revokedAt := int64(1_700_000_300_000)
	resp := postJSON(t, e.srv, "/v1/devices/"+second.id+"/revoke", rootToken, map[string]any{
		"revoked_at": revokedAt,
		"signature":  root.revokeSig(second.id, revokedAt),
	})
	resp.Body.Close()

	resp = postJSON(t, e.srv, "/v1/devices/"+second.id+"/revoke", rootToken, map[string]any{
		"revoked_at": revokedAt + 1,
		"signature":  root.revokeSig(second.id, revokedAt+1),
	})
	wantError(t, resp, http.StatusForbidden, "DEVICE_REVOKED")

	resp = postJSON(t, e.srv, "/v1/devices/FAKE_UNKNOWN/revoke", rootToken, map[string]any{
		"revoked_at": revokedAt,
		"signature":  root.revokeSig("FAKE_UNKNOWN", revokedAt),
	})
	wantError(t, resp, http.StatusNotFound, "NOT_FOUND")
}

func TestRevokedDeviceCannotPushRecords(t *testing.T) {
	t.Parallel()
	e, st := envWithStore(t)
	_, second := twoDeviceFixture(t, e, st)
	secondToken := authenticate(t, e, second)

	// Revoke directly in the store so the session token survives: the
	// per-request revocation re-check must still cut access
	// (belt-and-braces beyond session invalidation).
	err := st.AddRevocation(context.Background(), store.Revocation{
		AccountID: second.accountID,
		DeviceID:  second.id,
		Statement: []byte(`{"fake":"statement"}`),
		RevokedAt: 1_700_000_300_000,
		CreatedAt: 1_700_000_300_000,
	})
	if err != nil {
		t.Fatalf("seed revocation: %v", err)
	}

	resp := pushRecords(t, e, secondToken,
		second.signedRecord("FAKE_REC_1", "snippet", "FAKE_E1", 1, []byte("FAKE_CT"), nil))
	wantError(t, resp, http.StatusForbidden, "DEVICE_REVOKED")
}

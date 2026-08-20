package handler_test

import (
	"bytes"
	"crypto/ed25519"
	"crypto/rand"
	"encoding/json"
	"net/http"
	"testing"
)

func TestKeyUpdatesAreTargetedPerDevice(t *testing.T) {
	t.Parallel()
	e, st := envWithStore(t)
	root, second := twoDeviceFixture(t, e, st)
	rootToken := authenticate(t, e, root)
	secondToken := authenticate(t, e, second)

	payload := []byte("FAKE_SEALED_KEY_UPDATE")
	resp := postJSON(t, e.srv, "/v1/keys/updates", rootToken, map[string]any{
		"target_device_id": second.id,
		"payload":          payload,
	})
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("post key update: want 200, got %d", resp.StatusCode)
	}
	var posted struct {
		Seq int64 `json:"seq"`
	}
	decodeBody(t, resp, &posted)
	if posted.Seq != 1 {
		t.Fatalf("want seq 1, got %d", posted.Seq)
	}

	// The target device sees the message; the sender does not (targeted
	// pull, a device only reads its own messages).
	list := getPath(t, e.srv, "/v1/keys/updates?since=0", secondToken)
	defer list.Body.Close()
	var body struct {
		Updates []struct {
			Seq     int64  `json:"seq"`
			Payload []byte `json:"payload"`
		} `json:"updates"`
	}
	decodeBody(t, list, &body)
	if len(body.Updates) != 1 || !bytes.Equal(body.Updates[0].Payload, payload) {
		t.Fatalf("target must receive its update, got %+v", body.Updates)
	}

	own := getPath(t, e.srv, "/v1/keys/updates?since=0", rootToken)
	defer own.Body.Close()
	decodeBody(t, own, &body)
	if len(body.Updates) != 0 {
		t.Fatalf("sender must not see the targeted update, got %+v", body.Updates)
	}
}

func TestKeyUpdatePullAcknowledgesAndPrunesConsumedMessages(t *testing.T) {
	t.Parallel()
	e, st := envWithStore(t)
	root, second := twoDeviceFixture(t, e, st)
	rootToken := authenticate(t, e, root)
	secondToken := authenticate(t, e, second)

	for i := 0; i < 3; i++ {
		resp := postJSON(t, e.srv, "/v1/keys/updates", rootToken, map[string]any{
			"target_device_id": second.id,
			"payload":          []byte("FAKE_SEALED_KEY_UPDATE"),
		})
		resp.Body.Close()
	}

	var body struct {
		Updates []struct {
			Seq int64 `json:"seq"`
		} `json:"updates"`
	}
	// Acknowledging the first two leaves only the third to deliver, and the
	// acknowledged rows are gone from storage.
	list := getPath(t, e.srv, "/v1/keys/updates?since=2", secondToken)
	defer list.Body.Close()
	decodeBody(t, list, &body)
	if len(body.Updates) != 1 || body.Updates[0].Seq != 3 {
		t.Fatalf("want only seq 3 left, got %+v", body.Updates)
	}

	remaining, err := st.ListKeyUpdatesSince(t.Context(), root.accountID, second.id, 0)
	if err != nil {
		t.Fatalf("list stored updates: %v", err)
	}
	if len(remaining) != 1 || remaining[0].Seq != 3 {
		t.Fatalf("acknowledged messages must be dropped, got %+v", remaining)
	}

	// A device that never acknowledges keeps everything addressed to it.
	list2 := getPath(t, e.srv, "/v1/keys/updates?since=0", secondToken)
	defer list2.Body.Close()
	decodeBody(t, list2, &body)
	if len(body.Updates) != 1 {
		t.Fatalf("since=0 must not resurrect pruned rows, got %+v", body.Updates)
	}
}

func TestKeyUpdateRejectsRevokedAndUnknownTarget(t *testing.T) {
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

	resp = postJSON(t, e.srv, "/v1/keys/updates", rootToken, map[string]any{
		"target_device_id": second.id,
		"payload":          []byte("FAKE_SEALED_KEY_UPDATE"),
	})
	wantError(t, resp, http.StatusForbidden, "DEVICE_REVOKED")

	resp = postJSON(t, e.srv, "/v1/keys/updates", rootToken, map[string]any{
		"target_device_id": "FAKE_UNKNOWN",
		"payload":          []byte("FAKE_SEALED_KEY_UPDATE"),
	})
	wantError(t, resp, http.StatusNotFound, "NOT_FOUND")
}

// recoveryFixture registers an account and stores a recovery blob with a
// fresh rootproof keypair, returning the device and the rootproof keys.
func recoveryFixture(t *testing.T, e *env) (*device, ed25519.PublicKey, ed25519.PrivateKey) {
	t.Helper()
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)
	token := authenticate(t, e, d)

	proofPub, proofPriv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatalf("generate rootproof key: %v", err)
	}
	resp := doJSON(t, e.srv, http.MethodPut, "/v1/recovery", token, map[string]any{
		"blob":          []byte("FAKE_RECOVERY_BLOB_CONTENT"),
		"rootproof_pub": []byte(proofPub),
	})
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("put recovery: want 200, got %d", resp.StatusCode)
	}
	return d, proofPub, proofPriv
}

func TestRecoveryBlobRoundTripIsAnonymousButBackedOff(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d, _, _ := recoveryFixture(t, e)

	resp := getPath(t, e.srv, "/v1/recovery?account_id="+d.accountID, "")
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("get recovery: want 200, got %d", resp.StatusCode)
	}
	var body struct {
		Blob []byte `json:"blob"`
	}
	decodeBody(t, resp, &body)
	if !bytes.Equal(body.Blob, []byte("FAKE_RECOVERY_BLOB_CONTENT")) {
		t.Fatal("recovery blob mismatch")
	}

	// Immediate retry hits the escalating backoff.
	retry := getPath(t, e.srv, "/v1/recovery?account_id="+d.accountID, "")
	wantError(t, retry, http.StatusTooManyRequests, "RATE_LIMITED")
}

func TestRecoveryGetReturnsNotFoundWithoutBlob(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)

	resp := getPath(t, e.srv, "/v1/recovery?account_id="+d.accountID, "")
	wantError(t, resp, http.StatusNotFound, "NOT_FOUND")
}

func rootChallenge(t *testing.T, e *env, accountID string) []byte {
	t.Helper()
	resp := postJSON(t, e.srv, "/v1/root", "", map[string]any{"account_id": accountID})
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("root challenge: want 200, got %d", resp.StatusCode)
	}
	var body struct {
		Challenge []byte `json:"challenge"`
	}
	decodeBody(t, resp, &body)
	if len(body.Challenge) != 32 {
		t.Fatalf("root challenge: want 32 bytes, got %d", len(body.Challenge))
	}
	return body.Challenge
}

func rootProofSig(priv ed25519.PrivateKey, challenge []byte, accountID string) []byte {
	msg := []byte("typvia.rootproof.v1")
	msg = append(msg, challenge...)
	msg = append(msg, []byte(accountID)...)
	return ed25519.Sign(priv, msg)
}

func TestReRootReplacesTrustRootAndRevokesOldDevices(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	old, _, proofPriv := recoveryFixture(t, e)

	challenge := rootChallenge(t, e, old.accountID)
	newRoot := newDeviceKeys(t, "FAKE_DEV_RECOVERED")
	resp := postJSON(t, e.srv, "/v1/root", "", map[string]any{
		"account_id": old.accountID,
		"proof_sig":  rootProofSig(proofPriv, challenge, old.accountID),
		"new_root":   newRoot.rootBody(t),
	})
	resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("re-root: want 200, got %d", resp.StatusCode)
	}

	// The old device is revoked: challenge refused (old certificates
	// are invalid after the root swap).
	refused := postJSON(t, e.srv, "/v1/auth/challenge", "", map[string]any{"device_id": old.id})
	wantError(t, refused, http.StatusForbidden, "DEVICE_REVOKED")

	// The new root device authenticates and sees the swapped directory.
	newRoot.accountID = old.accountID
	token := authenticate(t, e, newRoot)
	dir := fetchDirectory(t, e, token)
	var rootDoc struct {
		DeviceID string `json:"device_id"`
	}
	if err := json.Unmarshal(dir.RootStatement, &rootDoc); err != nil {
		t.Fatalf("parse new root statement: %v", err)
	}
	if rootDoc.DeviceID != newRoot.id {
		t.Fatalf("want new root %s, got %s", newRoot.id, rootDoc.DeviceID)
	}
}

func TestReRootRejectsBadProofAndBurnsChallenge(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	old, _, proofPriv := recoveryFixture(t, e)

	challenge := rootChallenge(t, e, old.accountID)
	newRoot := newDeviceKeys(t, "FAKE_DEV_RECOVERED")

	// Wrong key signs the proof.
	_, wrongPriv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatalf("generate wrong key: %v", err)
	}
	resp := postJSON(t, e.srv, "/v1/root", "", map[string]any{
		"account_id": old.accountID,
		"proof_sig":  rootProofSig(wrongPriv, challenge, old.accountID),
		"new_root":   newRoot.rootBody(t),
	})
	wantError(t, resp, http.StatusBadRequest, "MALFORMED")

	// The challenge burned on the failed attempt: a correct proof over the
	// same challenge is refused until a new one is issued.
	resp = postJSON(t, e.srv, "/v1/root", "", map[string]any{
		"account_id": old.accountID,
		"proof_sig":  rootProofSig(proofPriv, challenge, old.accountID),
		"new_root":   newRoot.rootBody(t),
	})
	wantError(t, resp, http.StatusUnauthorized, "AUTH_CHALLENGE_EXPIRED")
}

func TestReRootIsUnavailableWithoutRegisteredProofKey(t *testing.T) {
	t.Parallel()
	e := newEnv(t)
	d := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, d)

	resp := postJSON(t, e.srv, "/v1/root", "", map[string]any{"account_id": d.accountID})
	wantError(t, resp, http.StatusNotFound, "NOT_FOUND")
}

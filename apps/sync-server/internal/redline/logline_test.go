// Log red line: server logs must never
// contain ciphertext bodies, signatures, session tokens, challenges,
// recovery blobs, or any request body content. This test swaps the default
// slog sink for a buffer, drives every endpoint through normal and error
// paths with FAKE_ marker payloads, then scans the captured bytes for the
// markers in raw, base64 and base64url forms.
package redline

import (
	"bytes"
	"context"
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/binary"
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"

	"typvia.dev/sync-server/internal/handler"
	"typvia.dev/sync-server/internal/service"
	"typvia.dev/sync-server/internal/store"
)

func TestLogsCarryNoPayloadContentOrCredentials(t *testing.T) {
	// Not parallel: the test owns the process-global default logger.
	var logBuf bytes.Buffer
	prev := slog.Default()
	slog.SetDefault(slog.New(slog.NewTextHandler(&logBuf, &slog.HandlerOptions{Level: slog.LevelDebug})))
	defer slog.SetDefault(prev)

	st, err := store.Open(filepath.Join(t.TempDir(), "redline.db"))
	if err != nil {
		t.Fatalf("open store: %v", err)
	}
	if err := st.Migrate(context.Background()); err != nil {
		t.Fatalf("migrate: %v", err)
	}
	srv := httptest.NewServer(handler.New(service.New(st)))
	defer srv.Close()

	secrets := driveScenario(t, srv)

	// A system-error path: the database is closed underneath the health
	// endpoint, forcing the one code path that logs an error cause.
	if err := st.Close(); err != nil {
		t.Fatalf("close store: %v", err)
	}
	resp, err := http.Get(srv.URL + "/healthz")
	if err != nil {
		t.Fatalf("get healthz: %v", err)
	}
	resp.Body.Close()

	logged := logBuf.Bytes()
	if len(logged) == 0 {
		t.Log("no log output captured; scanning is vacuous but the red line holds")
	}
	for name, secret := range secrets {
		for form, needle := range map[string][]byte{
			"raw":       secret,
			"base64":    []byte(base64.StdEncoding.EncodeToString(secret)),
			"base64url": []byte(base64.RawURLEncoding.EncodeToString(secret)),
		} {
			if len(needle) > 0 && bytes.Contains(logged, needle) {
				t.Errorf("log output contains %s (%s form): red line violated", name, form)
			}
		}
	}
}

// driveScenario exercises normal and error paths on every endpoint and
// returns the secret material that must never appear in logs.
func driveScenario(t *testing.T, srv *httptest.Server) map[string][]byte {
	t.Helper()
	pub, priv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatalf("generate key: %v", err)
	}
	xPub := make([]byte, 32)
	if _, err := rand.Read(xPub); err != nil {
		t.Fatalf("generate x25519 pub: %v", err)
	}
	const deviceID = "FAKE_LOGRL_DEVICE"
	createdAt := int64(1_700_000_000_000)

	rootSigned := buildRoot(deviceID, pub, xPub, "FAKE_LOGRL_NAME_MARKER", "macos", createdAt)
	rootSig := ed25519.Sign(priv, rootSigned)
	var created struct {
		AccountID string `json:"account_id"`
	}
	post(t, srv, "/v1/accounts", "", map[string]any{"root": map[string]any{
		"device_id": deviceID, "ed25519_pub": []byte(pub), "x25519_pub": xPub,
		"name": "FAKE_LOGRL_NAME_MARKER", "platform": "macos",
		"created_at": createdAt, "signature": rootSig,
	}}, http.StatusOK, &created)

	// Challenge–response, keeping challenge and token as secrets.
	var challengeOut struct {
		Challenge []byte `json:"challenge"`
	}
	post(t, srv, "/v1/auth/challenge", "", map[string]any{"device_id": deviceID}, http.StatusOK, &challengeOut)
	authMsg := append(append([]byte("typvia.auth.v1"), challengeOut.Challenge...), []byte(deviceID)...)
	var sessionOut struct {
		SessionToken string `json:"session_token"`
	}
	post(t, srv, "/v1/auth/session", "", map[string]any{
		"device_id": deviceID, "sig": ed25519.Sign(priv, authMsg),
	}, http.StatusOK, &sessionOut)
	token := sessionOut.SessionToken

	// Record push: success, then a bad signature, then a version conflict.
	ciphertext := []byte("FAKE_LOGRL_CIPHERTEXT_MARKER")
	recordSig := signRecord(priv, deviceID, "snippet", "FAKE_LOGRL_E1", 1, ciphertext)
	goodRecord := map[string]any{
		"id": "FAKE_LOGRL_REC1", "entity_type": "snippet", "entity_id": "FAKE_LOGRL_E1",
		"version": 1, "ciphertext": ciphertext, "updated_at": createdAt,
		"device_id": deviceID, "key_id": 1, "signature": recordSig,
	}
	post(t, srv, "/v1/records", token, map[string]any{"records": []any{goodRecord}}, http.StatusOK, nil)

	badRecord := map[string]any{}
	for k, v := range goodRecord {
		badRecord[k] = v
	}
	badRecord["id"] = "FAKE_LOGRL_REC2"
	badRecord["signature"] = bytes.Repeat([]byte{0x01}, 64)
	post(t, srv, "/v1/records", token, map[string]any{"records": []any{badRecord}}, http.StatusBadRequest, nil)

	staleRecord := map[string]any{}
	for k, v := range goodRecord {
		staleRecord[k] = v
	}
	staleRecord["id"] = "FAKE_LOGRL_REC3"
	staleRecord["signature"] = signRecord(priv, deviceID, "snippet", "FAKE_LOGRL_E1", 1, ciphertext)
	post(t, srv, "/v1/records", token, map[string]any{"records": []any{staleRecord}}, http.StatusConflict, nil)

	get(t, srv, "/v1/records?since=0", token)
	get(t, srv, "/v1/devices", token)

	// Pairing relay: begin, offer (with the certified new device), claim,
	// then a claim of a burned session.
	var pair struct {
		SessionID string `json:"session_id"`
	}
	post(t, srv, "/v1/pair/begin", "", map[string]any{"account_id": created.AccountID}, http.StatusOK, &pair)
	pairingPayload := []byte("FAKE_LOGRL_PAIRING_PAYLOAD_MARKER")
	newPub, _, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatalf("generate paired key: %v", err)
	}
	xNew := make([]byte, 32)
	if _, err := rand.Read(xNew); err != nil {
		t.Fatalf("generate paired x25519 pub: %v", err)
	}
	certSigned := buildCert("FAKE_LOGRL_PAIRED", newPub, xNew, "FAKE_LOGRL_PAIRED_NAME", "ios",
		createdAt, createdAt+1, deviceID)
	certSig := ed25519.Sign(priv, certSigned)
	post(t, srv, "/v1/pair/"+pair.SessionID+"/offer", token, map[string]any{
		"certificate": map[string]any{
			"device_id": "FAKE_LOGRL_PAIRED", "ed25519_pub": []byte(newPub), "x25519_pub": xNew,
			"name": "FAKE_LOGRL_PAIRED_NAME", "platform": "ios",
			"created_at": createdAt, "issued_at": createdAt + 1,
			"issuer_device_id": deviceID, "signature": certSig,
		},
		"sealed_bundle":    pairingPayload,
		"bundle_signature": bytes.Repeat([]byte{0x02}, 64),
	}, http.StatusOK, nil)
	post(t, srv, "/v1/pair/"+pair.SessionID+"/claim", "", nil, http.StatusOK, nil)
	post(t, srv, "/v1/pair/"+pair.SessionID+"/claim", "", nil, http.StatusNotFound, nil)

	// Key updates and recovery blob (plus its rate-limited retry).
	keyPayload := []byte("FAKE_LOGRL_KEYUPDATE_MARKER")
	post(t, srv, "/v1/keys/updates", token, map[string]any{
		"target_device_id": deviceID, "payload": keyPayload,
	}, http.StatusOK, nil)
	get(t, srv, "/v1/keys/updates?since=0", token)

	blob := []byte("FAKE_LOGRL_RECOVERY_BLOB_MARKER")
	proofPub, _, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatalf("generate rootproof key: %v", err)
	}
	put(t, srv, "/v1/recovery", token, map[string]any{"blob": blob, "rootproof_pub": []byte(proofPub)})
	get(t, srv, "/v1/recovery?account_id="+created.AccountID, "")
	get(t, srv, "/v1/recovery?account_id="+created.AccountID, "") // backoff path

	// Malformed body and unknown route error paths.
	postRaw(t, srv, "/v1/accounts", `{"FAKE_LOGRL_BROKEN_JSON":`)
	get(t, srv, "/v1/definitely-not-an-endpoint", "")

	return map[string][]byte{
		"ciphertext marker":         ciphertext,
		"record signature":          recordSig,
		"root signature":            rootSig,
		"session token":             []byte(token),
		"auth challenge":            challengeOut.Challenge,
		"pairing payload marker":    pairingPayload,
		"pairing cert signature":    certSig,
		"key update marker":         keyPayload,
		"recovery blob marker":      blob,
		"device name marker":        []byte("FAKE_LOGRL_NAME_MARKER"),
		"paired device name marker": []byte("FAKE_LOGRL_PAIRED_NAME"),
	}
}

func buildRoot(deviceID string, edPub, xPub []byte, name, platform string, createdAt int64) []byte {
	var b bytes.Buffer
	b.WriteString("typvia.root.v1")
	b.WriteString(deviceID)
	b.WriteByte(0)
	b.Write(edPub)
	b.Write(xPub)
	b.WriteString(name)
	b.WriteByte(0)
	b.WriteString(platform)
	b.WriteByte(0)
	le64(&b, uint64(createdAt))
	return b.Bytes()
}

func buildCert(deviceID string, edPub, xPub []byte, name, platform string, createdAt, issuedAt int64, issuerID string) []byte {
	var b bytes.Buffer
	b.WriteString("typvia.devcert.v1")
	b.WriteString(deviceID)
	b.WriteByte(0)
	b.Write(edPub)
	b.Write(xPub)
	b.WriteString(name)
	b.WriteByte(0)
	b.WriteString(platform)
	b.WriteByte(0)
	le64(&b, uint64(createdAt))
	le64(&b, uint64(issuedAt))
	b.WriteString(issuerID)
	b.WriteByte(0)
	return b.Bytes()
}

func signRecord(priv ed25519.PrivateKey, deviceID, entityType, entityID string, version int64, ciphertext []byte) []byte {
	var b bytes.Buffer
	b.WriteString("typvia.syncrec.v1")
	b.WriteString(deviceID)
	b.WriteByte(0)
	b.WriteString(entityType)
	b.WriteByte(0)
	b.WriteString(entityID)
	b.WriteByte(0)
	le64(&b, uint64(version))
	le64(&b, ^uint64(0))
	le64(&b, uint64(1_700_000_000_000))
	digest := sha256.Sum256(ciphertext)
	b.Write(digest[:])
	return ed25519.Sign(priv, b.Bytes())
}

func le64(b *bytes.Buffer, v uint64) {
	var buf [8]byte
	binary.LittleEndian.PutUint64(buf[:], v)
	b.Write(buf[:])
}

func post(t *testing.T, srv *httptest.Server, path, token string, body any, wantStatus int, dst any) {
	t.Helper()
	do(t, srv, http.MethodPost, path, token, body, wantStatus, dst)
}

func put(t *testing.T, srv *httptest.Server, path, token string, body any) {
	t.Helper()
	do(t, srv, http.MethodPut, path, token, body, http.StatusOK, nil)
}

func get(t *testing.T, srv *httptest.Server, path, token string) {
	t.Helper()
	req, err := http.NewRequest(http.MethodGet, srv.URL+path, nil)
	if err != nil {
		t.Fatalf("build request: %v", err)
	}
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("do request: %v", err)
	}
	resp.Body.Close()
}

func do(t *testing.T, srv *httptest.Server, method, path, token string, body any, wantStatus int, dst any) {
	t.Helper()
	var reader io.Reader
	if body != nil {
		raw, err := json.Marshal(body)
		if err != nil {
			t.Fatalf("marshal body: %v", err)
		}
		reader = bytes.NewReader(raw)
	}
	req, err := http.NewRequest(method, srv.URL+path, reader)
	if err != nil {
		t.Fatalf("build request: %v", err)
	}
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("do request: %v", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != wantStatus {
		t.Fatalf("%s %s: want %d, got %d", method, path, wantStatus, resp.StatusCode)
	}
	if dst != nil {
		if err := json.NewDecoder(resp.Body).Decode(dst); err != nil {
			t.Fatalf("decode response: %v", err)
		}
	}
}

func postRaw(t *testing.T, srv *httptest.Server, path, body string) {
	t.Helper()
	resp, err := http.Post(srv.URL+path, "application/json", bytes.NewReader([]byte(body)))
	if err != nil {
		t.Fatalf("post raw: %v", err)
	}
	resp.Body.Close()
}

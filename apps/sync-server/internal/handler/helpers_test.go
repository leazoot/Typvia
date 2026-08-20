package handler_test

import (
	"bytes"
	"context"
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"encoding/binary"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"sync"
	"testing"
	"time"

	"typvia.dev/sync-server/internal/handler"
	"typvia.dev/sync-server/internal/service"
	"typvia.dev/sync-server/internal/store"
)

// fakeTime is a mutable clock shared by the service and the rate limiters
// so tests can drive TTL and rate-limit windows deterministically.
type fakeTime struct {
	mu sync.Mutex
	t  time.Time
}

func newFakeTime() *fakeTime {
	return &fakeTime{t: time.Unix(1_700_000_000, 0)}
}

func (f *fakeTime) Now() time.Time {
	f.mu.Lock()
	defer f.mu.Unlock()
	return f.t
}

func (f *fakeTime) Advance(d time.Duration) {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.t = f.t.Add(d)
}

// env is a full three-layer test server with a controllable clock.
type env struct {
	srv   *httptest.Server
	clock *fakeTime
}

func newEnv(t *testing.T) *env {
	t.Helper()
	e, _ := envWithStore(t)
	return e
}

// device is a test-side device identity (FAKE keys generated per test).
type device struct {
	id        string
	pub       ed25519.PublicKey
	priv      ed25519.PrivateKey
	x25519Pub []byte
	accountID string
}

func newDeviceKeys(t *testing.T, id string) *device {
	t.Helper()
	pub, priv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatalf("generate device key: %v", err)
	}
	x := make([]byte, 32)
	if _, err := rand.Read(x); err != nil {
		t.Fatalf("generate x25519 pub: %v", err)
	}
	return &device{id: id, pub: pub, priv: priv, x25519Pub: x}
}

// rootBody builds the JSON root statement body self-signed by the device.
func (d *device) rootBody(t *testing.T) map[string]any {
	t.Helper()
	createdAt := int64(1_700_000_000_000)
	signed := rootSignedBytes(d.id, d.pub, d.x25519Pub, "FAKE_DEVICE_NAME", "macos", createdAt)
	return map[string]any{
		"device_id":   d.id,
		"ed25519_pub": d.pub,
		"x25519_pub":  d.x25519Pub,
		"name":        "FAKE_DEVICE_NAME",
		"platform":    "macos",
		"created_at":  createdAt,
		"signature":   ed25519.Sign(d.priv, signed),
	}
}

// rootSignedBytes rebuilds the root statement message test-side.
func rootSignedBytes(deviceID string, edPub, xPub []byte, name, platform string, createdAt int64) []byte {
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
	writeLE64(&b, uint64(createdAt))
	return b.Bytes()
}

func writeLE64(b *bytes.Buffer, v uint64) {
	var buf [8]byte
	binary.LittleEndian.PutUint64(buf[:], v)
	b.Write(buf[:])
}

// createAccount registers the device as a new account's root device.
func createAccount(t *testing.T, e *env, d *device) {
	t.Helper()
	resp := postJSON(t, e.srv, "/v1/accounts", "", map[string]any{"root": d.rootBody(t)})
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("create account: want 200, got %d", resp.StatusCode)
	}
	var body struct {
		AccountID string `json:"account_id"`
	}
	decodeBody(t, resp, &body)
	if body.AccountID == "" {
		t.Fatal("create account: empty account_id")
	}
	d.accountID = body.AccountID
}

// authenticate walks the full challenge–response exchange for the device.
func authenticate(t *testing.T, e *env, d *device) string {
	t.Helper()
	challenge := requestChallenge(t, e, d)
	sig := ed25519.Sign(d.priv, authSignedBytes(challenge, d.id))
	resp := postJSON(t, e.srv, "/v1/auth/session", "", map[string]any{"device_id": d.id, "sig": sig})
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("auth session: want 200, got %d", resp.StatusCode)
	}
	var body struct {
		SessionToken string `json:"session_token"`
	}
	decodeBody(t, resp, &body)
	if body.SessionToken == "" {
		t.Fatal("auth session: empty token")
	}
	return body.SessionToken
}

func requestChallenge(t *testing.T, e *env, d *device) []byte {
	t.Helper()
	resp := postJSON(t, e.srv, "/v1/auth/challenge", "", map[string]any{"device_id": d.id})
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("auth challenge: want 200, got %d", resp.StatusCode)
	}
	var body struct {
		Challenge []byte `json:"challenge"`
	}
	decodeBody(t, resp, &body)
	if len(body.Challenge) != 32 {
		t.Fatalf("auth challenge: want 32 bytes, got %d", len(body.Challenge))
	}
	return body.Challenge
}

func authSignedBytes(challenge []byte, deviceID string) []byte {
	msg := []byte("typvia.auth.v1")
	msg = append(msg, challenge...)
	msg = append(msg, []byte(deviceID)...)
	return msg
}

// signedRecord builds a record body signed by the device.
func (d *device) signedRecord(id, entityType, entityID string, version int64, ciphertext []byte, deletedAt *int64) map[string]any {
	updatedAt := int64(1_700_000_100_000)
	var b bytes.Buffer
	b.WriteString("typvia.syncrec.v1")
	b.WriteString(d.id)
	b.WriteByte(0)
	b.WriteString(entityType)
	b.WriteByte(0)
	b.WriteString(entityID)
	b.WriteByte(0)
	writeLE64(&b, uint64(version))
	if deletedAt != nil {
		writeLE64(&b, uint64(*deletedAt))
	} else {
		writeLE64(&b, ^uint64(0))
	}
	writeLE64(&b, uint64(updatedAt))
	digest := sha256.Sum256(ciphertext)
	b.Write(digest[:])

	body := map[string]any{
		"id":          id,
		"entity_type": entityType,
		"entity_id":   entityID,
		"version":     version,
		"updated_at":  updatedAt,
		"device_id":   d.id,
		"key_id":      1,
		"signature":   ed25519.Sign(d.priv, b.Bytes()),
	}
	if deletedAt != nil {
		body["deleted_at"] = *deletedAt
	} else {
		body["ciphertext"] = ciphertext
	}
	return body
}

// revokeSig signs a revocation statement for targetID.
func (d *device) revokeSig(targetID string, revokedAt int64) []byte {
	var b bytes.Buffer
	b.WriteString("typvia.revoke.v1")
	b.WriteString(targetID)
	writeLE64(&b, uint64(revokedAt))
	return ed25519.Sign(d.priv, b.Bytes())
}

// postJSON sends a JSON POST; token, when non-empty, becomes the bearer.
func postJSON(t *testing.T, srv *httptest.Server, path, token string, body any) *http.Response {
	t.Helper()
	return doJSON(t, srv, http.MethodPost, path, token, body)
}

func doJSON(t *testing.T, srv *httptest.Server, method, path, token string, body any) *http.Response {
	t.Helper()
	var reader io.Reader
	if body != nil {
		raw, err := json.Marshal(body)
		if err != nil {
			t.Fatalf("marshal request body: %v", err)
		}
		reader = bytes.NewReader(raw)
	}
	req, err := http.NewRequest(method, srv.URL+path, reader)
	if err != nil {
		t.Fatalf("build request: %v", err)
	}
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("do request: %v", err)
	}
	return resp
}

func getPath(t *testing.T, srv *httptest.Server, path, token string) *http.Response {
	t.Helper()
	return doJSON(t, srv, http.MethodGet, path, token, nil)
}

func decodeBody(t *testing.T, resp *http.Response, dst any) {
	t.Helper()
	if err := json.NewDecoder(resp.Body).Decode(dst); err != nil {
		t.Fatalf("decode response body: %v", err)
	}
}

// wantError asserts status and stable error code, closing the body.
func wantError(t *testing.T, resp *http.Response, status int, code string) {
	t.Helper()
	defer resp.Body.Close()
	if resp.StatusCode != status {
		t.Errorf("want status %d, got %d", status, resp.StatusCode)
	}
	if got := decodeErrorBody(t, resp); got != code {
		t.Errorf("want code %s, got %q", code, got)
	}
}

// twoDeviceFixture creates an account with its root device plus a second
// paired device registered directly in the store (the pairing relay does not
// register devices server-side; certificates travel opaquely).
func twoDeviceFixture(t *testing.T, e *env, st *store.Store) (*device, *device) {
	t.Helper()
	root := newDeviceKeys(t, "FAKE_DEV_ROOT")
	createAccount(t, e, root)
	second := newDeviceKeys(t, "FAKE_DEV_SECOND")
	second.accountID = root.accountID
	err := st.CreateDevice(context.Background(), store.Device{
		ID:         second.id,
		AccountID:  root.accountID,
		Name:       "FAKE_SECOND_NAME",
		Platform:   "ios",
		Ed25519Pub: second.pub,
		X25519Pub:  second.x25519Pub,
		CertChain:  []byte(`[{"fake":"cert"}]`),
		CreatedAt:  1,
	})
	if err != nil {
		t.Fatalf("register second device: %v", err)
	}
	return root, second
}

// envWithStore also exposes the store for fixtures needing direct seeding.
func envWithStore(t *testing.T) (*env, *store.Store) {
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
	clock := newFakeTime()
	svc := service.New(st, service.WithClock(clock.Now))
	srv := httptest.NewServer(handler.New(svc, handler.WithLimiterClock(clock.Now)))
	t.Cleanup(srv.Close)
	return &env{srv: srv, clock: clock}, st
}

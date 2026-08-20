// Package handler is the HTTP layer of the sync server: routing, request
// parsing, session middleware, rate limiting, and mapping service results to
// responses. No business logic and no SQL live here.
package handler

import (
	"net"
	"net/http"
	"strings"
	"time"

	"typvia.dev/sync-server/internal/limiter"
	"typvia.dev/sync-server/internal/service"
)

// protocolHeader carries the client protocol version.
const protocolHeader = "X-Typvia-Protocol"

// Request body caps. The records cap leaves room
// for base64 and JSON framing around the 4MB envelope-sum limit enforced in
// the service layer.
const (
	maxSmallBody   = 1 << 20 // 1MB: everything except record batches
	maxRecordsBody = 8 << 20 // 8MB transport cap around the 4MB batch limit
)

// api carries the wired dependencies of all endpoint handlers.
type api struct {
	svc *service.Service
	// challengeLimit smooths auth challenge requests per ip|device.
	challengeLimit *limiter.Bucket
	// recordsLimit is the deliberately generous per-device push/pull limit
	// (sync bursts are legitimate).
	recordsLimit *limiter.Bucket
	// recoveryBackoff escalates delays on the anonymous recovery endpoint
	// per ip|account (online brute-force resistance).
	recoveryBackoff *limiter.Backoff
}

// Option customizes the handler (test seams only).
type Option func(*api)

// WithLimiterClock replaces the rate-limiter clock, letting tests drive
// windows deterministically.
func WithLimiterClock(now func() time.Time) Option {
	return func(a *api) {
		a.challengeLimit = newChallengeLimit(now)
		a.recordsLimit = newRecordsLimit(now)
		a.recoveryBackoff = newRecoveryBackoff(now)
	}
}

func newChallengeLimit(now func() time.Time) *limiter.Bucket {
	// Burst 10, refill 12/min: far above legitimate re-auth, far below abuse.
	return limiter.NewBucket(10, 0.2, now)
}

func newRecordsLimit(now func() time.Time) *limiter.Bucket {
	// Burst 120, refill 10/s per device: generous baseline for sync bursts.
	return limiter.NewBucket(120, 10, now)
}

func newRecoveryBackoff(now func() time.Time) *limiter.Backoff {
	// 1s doubling to 5min, escalation resets after 1h of quiet.
	return limiter.NewBackoff(time.Second, 5*time.Minute, time.Hour, now)
}

// New builds the HTTP handler for the sync server API.
func New(svc *service.Service, opts ...Option) http.Handler {
	a := &api{
		svc:             svc,
		challengeLimit:  newChallengeLimit(time.Now),
		recordsLimit:    newRecordsLimit(time.Now),
		recoveryBackoff: newRecoveryBackoff(time.Now),
	}
	for _, opt := range opts {
		opt(a)
	}

	mux := http.NewServeMux()
	mux.HandleFunc("GET /healthz", func(w http.ResponseWriter, r *http.Request) {
		if err := svc.Health(r.Context()); err != nil {
			writeError(w, err)
			return
		}
		writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
	})
	mux.HandleFunc("GET /v1/handshake", func(w http.ResponseWriter, r *http.Request) {
		info, err := svc.Handshake(r.Header.Get(protocolHeader))
		if err != nil {
			writeError(w, err)
			return
		}
		writeJSON(w, http.StatusOK, info)
	})

	mux.HandleFunc("POST /v1/accounts", a.protocol(a.createAccount))
	mux.HandleFunc("POST /v1/auth/challenge", a.protocol(a.authChallenge))
	mux.HandleFunc("POST /v1/auth/session", a.protocol(a.authSession))
	mux.HandleFunc("POST /v1/records", a.protocol(a.auth(a.pushRecords)))
	mux.HandleFunc("GET /v1/records", a.protocol(a.auth(a.pullRecords)))
	mux.HandleFunc("GET /v1/devices", a.protocol(a.auth(a.deviceDirectory)))
	mux.HandleFunc("POST /v1/devices/{id}/revoke", a.protocol(a.auth(a.revokeDevice)))
	mux.HandleFunc("POST /v1/pair/begin", a.protocol(a.beginPairing))
	mux.HandleFunc("POST /v1/pair/{session}/offer", a.protocol(a.auth(a.pairingOffer)))
	mux.HandleFunc("POST /v1/pair/{session}/claim", a.protocol(a.pairingClaim))
	mux.HandleFunc("POST /v1/keys/updates", a.protocol(a.auth(a.addKeyUpdate)))
	mux.HandleFunc("GET /v1/keys/updates", a.protocol(a.auth(a.listKeyUpdates)))
	mux.HandleFunc("PUT /v1/recovery", a.protocol(a.auth(a.putRecovery)))
	mux.HandleFunc("GET /v1/recovery", a.protocol(a.getRecovery))
	mux.HandleFunc("POST /v1/root", a.protocol(a.reRoot))

	// Unknown routes get the same JSON error envelope as everything else.
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		writeError(w, service.NotFoundErr("unknown endpoint"))
	})
	return mux
}

// protocol validates the X-Typvia-Protocol header when present: the same
// window check as the handshake; absent headers pass for discovery parity.
func (a *api) protocol(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if _, err := a.svc.Handshake(r.Header.Get(protocolHeader)); err != nil {
			writeError(w, err)
			return
		}
		next(w, r)
	}
}

// authedHandler is an endpoint requiring a device session.
type authedHandler func(w http.ResponseWriter, r *http.Request, sess service.Session)

// auth resolves the Authorization bearer token to a device session. The
// token never appears in URLs or logs.
func (a *api) auth(next authedHandler) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		token, ok := strings.CutPrefix(r.Header.Get("Authorization"), "Bearer ")
		if !ok {
			writeError(w, &service.Error{
				Kind: service.KindBusiness, Code: service.CodeSessionExpired,
				Message: "missing bearer token",
			})
			return
		}
		sess, err := a.svc.Authenticate(r.Context(), token)
		if err != nil {
			writeError(w, err)
			return
		}
		next(w, r, sess)
	}
}

// clientIP extracts the remote IP for rate-limit keys. The direct peer
// address is used; deployments behind a reverse proxy share its address,
// which only makes the limits stricter, never looser.
func clientIP(r *http.Request) string {
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		return r.RemoteAddr
	}
	return host
}

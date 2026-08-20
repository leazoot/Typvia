package handler

import (
	"net/http"

	"typvia.dev/sync-server/internal/service"
)

// rootDeviceBody is the JSON form of a self-signed root statement;
// []byte fields are base64 in JSON.
type rootDeviceBody struct {
	DeviceID   string `json:"device_id"`
	Ed25519Pub []byte `json:"ed25519_pub"`
	X25519Pub  []byte `json:"x25519_pub"`
	Name       string `json:"name"`
	Platform   string `json:"platform"`
	CreatedAt  int64  `json:"created_at"`
	Signature  []byte `json:"signature"`
}

func (b rootDeviceBody) toInput() service.RootDeviceInput {
	return service.RootDeviceInput{
		DeviceID:   b.DeviceID,
		Ed25519Pub: b.Ed25519Pub,
		X25519Pub:  b.X25519Pub,
		Name:       b.Name,
		Platform:   b.Platform,
		CreatedAt:  b.CreatedAt,
		Signature:  b.Signature,
	}
}

// createAccount handles POST /v1/accounts (first device registration).
func (a *api) createAccount(w http.ResponseWriter, r *http.Request) {
	var body struct {
		Root rootDeviceBody `json:"root"`
	}
	if err := decodeJSON(w, r, maxSmallBody, &body); err != nil {
		writeError(w, err)
		return
	}
	accountID, err := a.svc.CreateAccount(r.Context(), body.Root.toInput())
	if err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"account_id": accountID})
}

// authChallenge handles POST /v1/auth/challenge (step 1), rate limited
// per ip|device.
func (a *api) authChallenge(w http.ResponseWriter, r *http.Request) {
	var body struct {
		DeviceID string `json:"device_id"`
	}
	if err := decodeJSON(w, r, maxSmallBody, &body); err != nil {
		writeError(w, err)
		return
	}
	if !a.challengeLimit.Allow(clientIP(r) + "|" + body.DeviceID) {
		writeError(w, service.RateLimitedErr("too many challenge requests; retry later"))
		return
	}
	out, err := a.svc.IssueChallenge(r.Context(), body.DeviceID)
	if err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"challenge":  out.Challenge,
		"expires_in": out.ExpiresIn,
	})
}

// authSession handles POST /v1/auth/session (step 2).
func (a *api) authSession(w http.ResponseWriter, r *http.Request) {
	var body struct {
		DeviceID string `json:"device_id"`
		Sig      []byte `json:"sig"`
	}
	if err := decodeJSON(w, r, maxSmallBody, &body); err != nil {
		writeError(w, err)
		return
	}
	out, err := a.svc.CreateSession(r.Context(), body.DeviceID, body.Sig)
	if err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"session_token": out.SessionToken,
		"expires_in":    out.ExpiresIn,
	})
}

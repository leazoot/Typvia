package handler

import (
	"net/http"

	"typvia.dev/sync-server/internal/service"
)

// deviceDirectory handles GET /v1/devices: the device directory with
// certificate chains and revocation statements.
func (a *api) deviceDirectory(w http.ResponseWriter, r *http.Request, sess service.Session) {
	out, err := a.svc.DeviceDirectory(r.Context(), sess)
	if err != nil {
		writeError(w, err)
		return
	}
	if out.Devices == nil {
		out.Devices = []service.DeviceOut{}
	}
	if out.Revocations == nil {
		out.Revocations = []service.RevocationOut{}
	}
	writeJSON(w, http.StatusOK, out)
}

// revokeDevice handles POST /v1/devices/{id}/revoke.
func (a *api) revokeDevice(w http.ResponseWriter, r *http.Request, sess service.Session) {
	var body struct {
		RevokedAt int64  `json:"revoked_at"`
		Signature []byte `json:"signature"`
	}
	if err := decodeJSON(w, r, maxSmallBody, &body); err != nil {
		writeError(w, err)
		return
	}
	err := a.svc.RevokeDevice(r.Context(), sess, r.PathValue("id"), body.RevokedAt, body.Signature)
	if err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"status": "revoked"})
}

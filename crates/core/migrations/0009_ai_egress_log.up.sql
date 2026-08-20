-- AI egress audit log.
-- Device-local, append-only: one row per AI request that actually left
-- this device — when, to which provider, which class, how many body
-- bytes. Red line: the table structurally cannot hold prompt
-- content, response content or key material; these columns are all it has.
--
-- AUTOINCREMENT keeps ids monotonic and never reused (audit semantics).
-- Deliberately NO foreign key onto ai_provider: the log must survive
-- provider deletion — deleting a provider must not erase the audit trail.
CREATE TABLE ai_egress_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    request_class TEXT NOT NULL,
    request_bytes INTEGER NOT NULL CHECK (request_bytes >= 0)
);

CREATE INDEX idx_ai_egress_log_occurred_at ON ai_egress_log(occurred_at);

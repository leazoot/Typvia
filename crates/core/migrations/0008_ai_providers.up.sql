-- AI provider configuration.
-- Device-local entity: never part of the sync entity set. Red line: no
-- key column exists here — the API key lives only in the platform secure
-- store (ai.api_key.<provider_id>).
--
-- ai_action.provider_id deliberately gets NO foreign key onto this table:
-- ai_action is a syncable entity while ai_provider is device-local, so an
-- action applied from a remote device may legitimately reference a
-- provider this machine has not configured. The execution chain surfaces
-- that as a "provider not configured" user error.
CREATE TABLE ai_provider (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    base_url TEXT NOT NULL,
    model TEXT NOT NULL,
    timeout_ms INTEGER NOT NULL CHECK (timeout_ms > 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_ai_action_provider_id ON ai_action(provider_id);

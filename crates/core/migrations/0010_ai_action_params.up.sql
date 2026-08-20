-- AI action provider parameters + app-level bootstrap markers.
--
-- `params` is a JSON object of provider call parameters.
-- Known keys are validated in the model layer; unknown keys
-- survive read-modify-write round trips (forward compatibility).
ALTER TABLE ai_action ADD COLUMN params TEXT NOT NULL DEFAULT '{}';

-- Device-local one-shot bootstrap markers (first key: ai_actions_seeded).
-- Not a configuration store: user-facing settings stay in entity tables.
CREATE TABLE app_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

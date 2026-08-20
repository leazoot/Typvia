-- Local semantic-search vectors.
-- Derived data: regenerable from the local model at any time; never enters
-- sync payloads or snapshots. Red line (stricter than FTS): sensitive rows
-- are never embedded — enforced in the write layer (repository refuses
-- non-normal security levels) and asserted by regression tests; the schema
-- itself cannot see security_level without a trigger, which ad-hoc DDL
-- rules exclude.
CREATE TABLE snippet_embedding (
    snippet_id TEXT PRIMARY KEY REFERENCES snippet(id) ON DELETE CASCADE,
    model_id TEXT NOT NULL,
    dims INTEGER NOT NULL CHECK (dims > 0),
    vector BLOB NOT NULL,
    embedded_at INTEGER NOT NULL
);

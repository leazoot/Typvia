-- Recycle bin (soft delete) and snippet version history (PRD §12.16).

-- Soft-delete marker; NULL = live, set = in the recycle bin.
ALTER TABLE snippet ADD COLUMN deleted_at INTEGER;

CREATE INDEX idx_snippet_deleted_at ON snippet(deleted_at)
    WHERE deleted_at IS NOT NULL;

-- Append-only content history for snippets. Restore never rewrites rows:
-- it appends the restored state as a new version.
CREATE TABLE snippet_version (
    id TEXT PRIMARY KEY,
    snippet_id TEXT NOT NULL REFERENCES snippet(id) ON DELETE CASCADE,
    version INTEGER NOT NULL CHECK (version >= 1),
    title TEXT NOT NULL,
    content_plaintext TEXT,
    content_ciphertext BLOB,
    created_at INTEGER NOT NULL,
    UNIQUE (snippet_id, version),
    -- Same body-pair rule as the snippet table.
    CHECK ((content_plaintext IS NULL) <> (content_ciphertext IS NULL))
);

CREATE INDEX idx_snippet_version_snippet_id ON snippet_version(snippet_id);

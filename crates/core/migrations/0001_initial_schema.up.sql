-- Initial schema covering every PRD §15 persisted entity.
-- KeyboardSnapshot is an exported JSON document, not a table.

CREATE TABLE folder (
    id TEXT PRIMARY KEY,
    parent_id TEXT REFERENCES folder(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (parent_id IS NULL OR parent_id <> id)
);

CREATE TABLE tag (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);

CREATE TABLE snippet (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content_plaintext TEXT,
    content_ciphertext BLOB,
    type TEXT NOT NULL,
    description TEXT,
    folder_id TEXT REFERENCES folder(id) ON DELETE SET NULL,
    "trigger" TEXT,
    trigger_mode TEXT,
    language TEXT,
    security_level TEXT NOT NULL,
    is_favorite INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1)),
    is_pinned INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0, 1)),
    is_enabled INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0, 1)),
    platform_scope TEXT NOT NULL DEFAULT '[]',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_used_at INTEGER,
    usage_count INTEGER NOT NULL DEFAULT 0 CHECK (usage_count >= 0),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
    -- Exactly one body column is set per row.
    CHECK ((content_plaintext IS NULL) <> (content_ciphertext IS NULL)),
    -- Red line: sensitive rows never carry a plaintext body.
    CHECK (security_level <> 'sensitive' OR content_plaintext IS NULL),
    -- Trigger and its mode are set together or not at all.
    CHECK (("trigger" IS NULL) = (trigger_mode IS NULL))
);

CREATE INDEX idx_snippet_folder_id ON snippet(folder_id);
CREATE INDEX idx_snippet_trigger ON snippet("trigger") WHERE "trigger" IS NOT NULL;
CREATE INDEX idx_snippet_updated_at ON snippet(updated_at);
CREATE INDEX idx_snippet_last_used_at ON snippet(last_used_at);

CREATE TABLE snippet_tag (
    snippet_id TEXT NOT NULL REFERENCES snippet(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
    PRIMARY KEY (snippet_id, tag_id)
) WITHOUT ROWID;

CREATE TABLE template_field (
    id TEXT PRIMARY KEY,
    snippet_id TEXT NOT NULL REFERENCES snippet(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    label TEXT NOT NULL,
    type TEXT NOT NULL,
    default_value TEXT,
    options TEXT NOT NULL DEFAULT '[]',
    validation TEXT,
    is_required INTEGER NOT NULL DEFAULT 0 CHECK (is_required IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0,
    platform_overrides TEXT,
    UNIQUE (snippet_id, name)
);

CREATE TABLE app_rule (
    id TEXT PRIMARY KEY,
    snippet_id TEXT NOT NULL REFERENCES snippet(id) ON DELETE CASCADE,
    platform TEXT NOT NULL,
    app_identifier TEXT NOT NULL,
    rule_type TEXT NOT NULL,
    window_title_pattern TEXT
);

CREATE INDEX idx_app_rule_snippet_id ON app_rule(snippet_id);

CREATE TABLE device (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    platform TEXT NOT NULL,
    public_key BLOB NOT NULL,
    trust_level TEXT NOT NULL,
    last_seen_at INTEGER,
    created_at INTEGER NOT NULL,
    revoked_at INTEGER,
    -- Revoked devices have a revocation time; trusted devices do not.
    CHECK ((trust_level = 'revoked') = (revoked_at IS NOT NULL))
);

CREATE TABLE sync_record (
    id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    ciphertext BLOB,
    deleted_at INTEGER,
    updated_at INTEGER NOT NULL,
    device_id TEXT NOT NULL REFERENCES device(id),
    UNIQUE (entity_type, entity_id, version),
    -- Content records carry ciphertext; tombstones carry none.
    CHECK ((deleted_at IS NULL) = (ciphertext IS NOT NULL))
);

CREATE TABLE ai_action (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    prompt_template TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    model TEXT NOT NULL,
    input_source TEXT NOT NULL,
    output_mode TEXT NOT NULL,
    permission_scope TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Full-text index over snippet search fields (FR-2). Sensitive snippets
-- contribute title/tags/description only; population and the red-line tests
-- guarding it belong to the search crate (TASK-019/021).
CREATE VIRTUAL TABLE snippet_fts USING fts5(
    snippet_id UNINDEXED,
    title,
    content,
    description,
    tags,
    folder_name,
    "trigger",
    tokenize = 'unicode61'
);

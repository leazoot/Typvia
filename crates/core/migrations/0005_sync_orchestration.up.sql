-- Client-side sync orchestration state. No key material lives in any of these
-- tables; plaintext appears only in shadow documents, at the same
-- sensitivity as the entity tables themselves (sensitive bodies stay
-- K_vault ciphertext inside the document).

-- Outbox lifecycle on the existing sync_record table.
ALTER TABLE sync_record ADD COLUMN key_id INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sync_record ADD COLUMN signature BLOB NOT NULL DEFAULT x'';
ALTER TABLE sync_record ADD COLUMN state TEXT NOT NULL DEFAULT 'pending';
ALTER TABLE sync_record ADD COLUMN server_seq INTEGER;

CREATE INDEX idx_sync_record_state ON sync_record(state);

-- Shadow base document and applied head version per synced entity.
-- The row survives tombstone application (document goes NULL) so the head
-- version keeps guarding against revival by stale records.
CREATE TABLE sync_shadow (
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    document BLOB,
    PRIMARY KEY (entity_type, entity_id)
);

-- Remote records that cannot be applied yet (unknown K_sync generation,
-- payload version ahead, entity type without a local apply path). Kept as
-- full wire bytes so re-application runs the complete verification again.
CREATE TABLE sync_pending_record (
    id TEXT PRIMARY KEY,
    server_seq INTEGER NOT NULL UNIQUE,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    ciphertext BLOB,
    deleted_at INTEGER,
    updated_at INTEGER NOT NULL,
    device_id TEXT NOT NULL,
    key_id INTEGER NOT NULL,
    signature BLOB NOT NULL,
    reason TEXT NOT NULL,
    received_at INTEGER NOT NULL,
    -- Tombstones carry no ciphertext, content records must.
    CHECK ((deleted_at IS NULL) = (ciphertext IS NOT NULL))
);

-- Single-row sync configuration and pull cursor. Holds no key material:
-- sync_key_id is only the highest active K_sync generation number; the key
-- itself lives in the platform secure store.
CREATE TABLE sync_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    server_url TEXT,
    account_id TEXT,
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    applied_server_seq INTEGER NOT NULL DEFAULT 0,
    sync_key_id INTEGER NOT NULL DEFAULT 0,
    root_fingerprint BLOB,
    updated_at INTEGER NOT NULL DEFAULT 0
);

INSERT INTO sync_config (id) VALUES (1);

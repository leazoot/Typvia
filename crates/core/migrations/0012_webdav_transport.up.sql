-- WebDAV storage form. transport_kind selects the sync backend; webdav_push_seq is
-- this device's own published-record counter; webdav_cursor holds the
-- per-source-device pull cursors that replace the global server_seq
-- watermark in the WebDAV form. No key or content columns anywhere.

ALTER TABLE sync_config ADD COLUMN transport_kind TEXT NOT NULL DEFAULT 'server';
ALTER TABLE sync_config ADD COLUMN webdav_push_seq INTEGER NOT NULL DEFAULT 0;

CREATE TABLE webdav_cursor (
    source_device_id TEXT PRIMARY KEY,
    next_seq INTEGER NOT NULL DEFAULT 1
) WITHOUT ROWID;

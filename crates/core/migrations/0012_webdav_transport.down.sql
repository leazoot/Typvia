-- Reverts 0012_webdav_transport.up.sql.

DROP TABLE webdav_cursor;
ALTER TABLE sync_config DROP COLUMN webdav_push_seq;
ALTER TABLE sync_config DROP COLUMN transport_kind;

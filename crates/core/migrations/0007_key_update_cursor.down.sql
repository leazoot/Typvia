-- Reverts 0007_key_update_cursor.up.sql.

ALTER TABLE sync_config DROP COLUMN key_update_seq;

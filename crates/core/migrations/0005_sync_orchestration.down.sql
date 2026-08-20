-- Reverts 0005_sync_orchestration.up.sql.

DROP TABLE sync_config;
DROP TABLE sync_pending_record;
DROP TABLE sync_shadow;

DROP INDEX idx_sync_record_state;

ALTER TABLE sync_record DROP COLUMN server_seq;
ALTER TABLE sync_record DROP COLUMN state;
ALTER TABLE sync_record DROP COLUMN signature;
ALTER TABLE sync_record DROP COLUMN key_id;

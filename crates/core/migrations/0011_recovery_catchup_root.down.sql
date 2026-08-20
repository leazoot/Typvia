-- Reverts 0011_recovery_catchup_root.up.sql.

ALTER TABLE sync_config DROP COLUMN recovery_root;

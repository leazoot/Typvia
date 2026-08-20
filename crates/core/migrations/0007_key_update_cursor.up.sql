-- Key-update cursor. The device persists how far it has consumed the sealed
-- device-to-device key messages; the same value is sent as `since` on the
-- next pull, which the server reads as an acknowledgment and uses to drop
-- the messages this device is done with.
ALTER TABLE sync_config ADD COLUMN key_update_seq INTEGER NOT NULL DEFAULT 0;

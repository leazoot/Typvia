-- Recovery catch-up marker. While a
-- post-recovery historical catch-up is incomplete, this column carries the
-- predecessor trust-root statement (public signed JSON — never key material)
-- so an interrupted catch-up can resume across restarts; cleared when the
-- catch-up drains the server. NULL means nothing is pending.
ALTER TABLE sync_config ADD COLUMN recovery_root BLOB;

-- Rollback of recycle bin and version history.

DROP TABLE snippet_version;
DROP INDEX idx_snippet_deleted_at;
ALTER TABLE snippet DROP COLUMN deleted_at;

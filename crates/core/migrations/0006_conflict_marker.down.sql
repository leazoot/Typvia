-- Reverts 0006_conflict_marker.up.sql.

DROP INDEX idx_snippet_conflict_of;

ALTER TABLE snippet DROP COLUMN conflict_of;

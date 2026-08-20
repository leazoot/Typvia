-- Conflict-copy marker for sync body conflicts. A snippet created as the
-- "local version" of a concurrent body edit points back at the source
-- snippet; the marker travels inside the sync payload document so both
-- devices see the pending resolution. Deleting the source keeps the copy
-- but clears the marker (SET NULL).
ALTER TABLE snippet ADD COLUMN conflict_of TEXT REFERENCES snippet(id) ON DELETE SET NULL;

CREATE INDEX idx_snippet_conflict_of ON snippet(conflict_of);

-- Supports the Library's reader-chosen orders ("recently used", "recently
-- added") in every scope, not only in the saved views that happened to be
-- ordered that way. Each index carries the list's own tiebreaks so it covers
-- the whole ORDER BY; a missing trailing column sends SQLite back to a
-- temporary b-tree over the entire library at the 50k target.
CREATE INDEX idx_snippet_last_used_order ON snippet(last_used_at DESC, updated_at DESC, id);
CREATE INDEX idx_snippet_created_order ON snippet(created_at DESC, id);

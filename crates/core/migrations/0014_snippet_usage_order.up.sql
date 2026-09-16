-- Supports the "most used" list.
-- The existing last_used_at index answers "what did I reach for last"; this
-- one answers "what do I reach for", which is a different ordering and would
-- otherwise scan the whole table at the 50k target. The trailing column is
-- the list's own tiebreaks, so the index covers the whole ORDER BY. Leaving
-- the last one out is not free: SQLite falls back to a temporary b-tree for
-- it, which is the cost this index exists to avoid.
CREATE INDEX idx_snippet_usage_count ON snippet(usage_count DESC, last_used_at DESC, id);

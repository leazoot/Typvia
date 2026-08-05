-- Restores the version-1 shape of the FTS table (without language). The
-- index content is derivable data; a rebuild repopulates it after rollback.

DROP TABLE snippet_fts;

CREATE VIRTUAL TABLE snippet_fts USING fts5(
    snippet_id UNINDEXED,
    title,
    content,
    description,
    tags,
    folder_name,
    "trigger",
    tokenize = 'unicode61'
);

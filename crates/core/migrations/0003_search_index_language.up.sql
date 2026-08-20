-- Code language is one of the searchable fields, but the initial FTS
-- table missed that column. FTS5 virtual tables cannot be ALTERed, so the
-- table is recreated. No data is lost: index population first ships with the
-- search crate at this same schema version, so the table is empty before it.

DROP TABLE snippet_fts;

CREATE VIRTUAL TABLE snippet_fts USING fts5(
    snippet_id UNINDEXED,
    title,
    content,
    description,
    tags,
    folder_name,
    "trigger",
    language,
    tokenize = 'unicode61'
);

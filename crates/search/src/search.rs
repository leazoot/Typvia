//! Weighted search over the FTS index (PRD §12.2).
//!
//! Ranking is two-staged: FTS5 retrieves up to [`CANDIDATE_LIMIT`]
//! bm25-ordered candidates (column weights favor title/trigger/tags), then
//! Rust assigns each a [`MatchTier`] and orders by tier, last used, usage
//! count. App relevance (PRD §12.2 #6) and semantic similarity (#9) are
//! deferred to later batches: their rank slots sit between `Content` and the
//! recency tiebreak and will extend the sort key here, not the FTS query.

use rusqlite::{Connection, params};

use crate::error::SearchError;
use crate::query::{parse_match_expr, query_terms_lower};

/// How a hit matched, in descending rank priority (declaration order is the
/// sort order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchTier {
    /// The whole query equals the title (case-insensitive).
    TitleExact,
    /// The query matches the trigger (exact, or prefix of the trigger with
    /// leading punctuation like `:` ignored).
    Trigger,
    /// The title starts with the query.
    TitlePrefix,
    /// Every query term appears in the snippet's tag names.
    Tag,
    /// Matched in the remaining indexed fields (content, description,
    /// folder name, language).
    Content,
}

/// One ranked search result.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub snippet_id: String,
    pub title: String,
    pub tier: MatchTier,
}

/// Read-side companion of `SearchIndex`: parses queries and ranks hits.
pub struct Searcher<'c> {
    conn: &'c Connection,
}

/// Upper bound on candidates re-ranked in Rust. bm25 column weights push
/// title/trigger/tag matches to the front, so top-tier hits cannot
/// realistically fall outside this window even at the 50k target scale.
const CANDIDATE_LIMIT: usize = 512;

/// bm25 weight per FTS column, in table column order:
/// snippet_id (unindexed), title, content, description, tags, folder_name,
/// trigger, language.
const CANDIDATE_SQL: &str = "SELECT snippet_fts.snippet_id, s.title, s.\"trigger\",
        (SELECT group_concat(t.name, ' ')
           FROM snippet_tag st JOIN tag t ON t.id = st.tag_id
          WHERE st.snippet_id = s.id),
        s.last_used_at, s.usage_count
   FROM snippet_fts
   JOIN snippet s ON s.id = snippet_fts.snippet_id
  WHERE snippet_fts MATCH ?1 AND s.deleted_at IS NULL
  ORDER BY bm25(snippet_fts, 0.0, 10.0, 1.0, 2.0, 5.0, 2.0, 8.0, 2.0)
  LIMIT ?2";

struct Candidate {
    snippet_id: String,
    title: String,
    trigger: Option<String>,
    tags: Option<String>,
    last_used_at: Option<i64>,
    usage_count: i64,
}

impl<'c> Searcher<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Runs a weighted search. A query without indexable terms yields no
    /// hits. `limit`/`offset` paginate the ranked result.
    pub fn search(
        &self,
        query: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<SearchHit>, SearchError> {
        let Some(expr) = parse_match_expr(query) else {
            return Ok(Vec::new());
        };
        let terms = query_terms_lower(query);
        let normalized_query = terms.join(" ");

        let mut stmt = self.conn.prepare_cached(CANDIDATE_SQL)?;
        let rows = stmt.query_map(params![expr, CANDIDATE_LIMIT as i64], |row| {
            Ok(Candidate {
                snippet_id: row.get(0)?,
                title: row.get(1)?,
                trigger: row.get(2)?,
                tags: row.get(3)?,
                last_used_at: row.get(4)?,
                usage_count: row.get(5)?,
            })
        })?;

        let mut ranked: Vec<(MatchTier, Candidate)> = Vec::new();
        for row in rows {
            let candidate = row?;
            let tier = classify(&normalized_query, &terms, &candidate);
            ranked.push((tier, candidate));
        }

        ranked.sort_by(|(tier_a, a), (tier_b, b)| {
            tier_a
                .cmp(tier_b)
                .then(b.last_used_at.cmp(&a.last_used_at))
                .then(b.usage_count.cmp(&a.usage_count))
                .then(a.title.cmp(&b.title))
                .then(a.snippet_id.cmp(&b.snippet_id))
        });

        Ok(ranked
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(|(tier, c)| SearchHit {
                snippet_id: c.snippet_id,
                title: c.title,
                tier,
            })
            .collect())
    }
}

/// Assigns the highest tier the candidate qualifies for. `normalized_query`
/// is the lowercased query with whitespace collapsed to single spaces.
fn classify(normalized_query: &str, terms_lower: &[String], c: &Candidate) -> MatchTier {
    let title_lower = c.title.to_lowercase();
    if title_lower == normalized_query {
        return MatchTier::TitleExact;
    }
    if let Some(trigger) = &c.trigger {
        let trigger_lower = trigger.to_lowercase();
        let stripped = trigger_lower.trim_start_matches(|ch: char| !ch.is_alphanumeric());
        if trigger_lower == normalized_query || stripped.starts_with(normalized_query) {
            return MatchTier::Trigger;
        }
    }
    if title_lower.starts_with(normalized_query) {
        return MatchTier::TitlePrefix;
    }
    if let Some(tags) = &c.tags {
        let tags_lower = tags.to_lowercase();
        if terms_lower.iter().all(|t| tags_lower.contains(t.as_str())) {
            return MatchTier::Tag;
        }
    }
    MatchTier::Content
}

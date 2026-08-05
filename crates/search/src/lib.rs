//! Local full-text search for Typvia: FTS5 index maintenance and weighted
//! ranking over the `snippet_fts` virtual table owned by the core schema.
//!
//! Red line: sensitive snippets contribute only title, tags and description
//! to the index — never body, and the ciphertext column is never even read
//! here (docs/PRD.md §12.2, .claude/rules/database.md).

mod error;
mod index;
mod query;
mod search;
pub mod segment;

pub use error::SearchError;
pub use index::SearchIndex;
pub use search::{MatchTier, SearchHit, Searcher};

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Local full-text search for Typvia: FTS5 index maintenance and weighted
//! ranking over the `snippet_fts` virtual table owned by the core schema.
//!
//! Red line: sensitive snippets contribute only title, tags and description
//! to the index — never body, and the ciphertext column is never even read
//! here.

mod error;
mod index;
mod query;
mod search;
pub mod segment;

pub use error::SearchError;
pub use index::SearchIndex;
pub use search::{MatchTier, SearchHit, Searcher};

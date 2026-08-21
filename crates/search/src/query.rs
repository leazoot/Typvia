// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Query parsing: whitespace-separated terms combined with implicit AND.
//!
//! Every term matches as a phrase (CJK-split per the segment module), and
//! the last term additionally matches by prefix so results narrow while the
//! user is still typing. Deferred pinyin and fuzzy expansion would plug in
//! here — this is the single place query terms are produced.

use crate::segment::query_phrase;

/// Builds the FTS5 MATCH expression for a raw user query. Returns `None`
/// when the query holds no indexable term (empty, whitespace, punctuation
/// only) — such a query matches nothing rather than erroring in FTS5.
pub(crate) fn parse_match_expr(query: &str) -> Option<String> {
    let terms: Vec<&str> = query
        .split_whitespace()
        .filter(|t| t.chars().any(char::is_alphanumeric))
        .collect();
    let (last, init) = terms.split_last()?;
    let mut expr = String::new();
    for term in init {
        expr.push_str(&query_phrase(term));
        expr.push(' ');
    }
    expr.push_str(&query_phrase(last));
    expr.push('*');
    Some(expr)
}

/// Lowercased terms of a query, used by ranking classification.
pub(crate) fn query_terms_lower(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .filter(|t| t.chars().any(char::is_alphanumeric))
        .map(str::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_term_becomes_a_prefix_phrase() {
        assert_eq!(parse_match_expr("deploy").as_deref(), Some("\"deploy\"*"));
    }

    #[test]
    fn only_the_last_term_gets_the_prefix() {
        assert_eq!(
            parse_match_expr("docker rest").as_deref(),
            Some("\"docker\" \"rest\"*")
        );
    }

    #[test]
    fn cjk_terms_become_split_phrases() {
        assert_eq!(parse_match_expr("发票").as_deref(), Some("\"发 票\"*"));
    }

    #[test]
    fn blank_or_punctuation_only_queries_parse_to_none() {
        assert_eq!(parse_match_expr(""), None);
        assert_eq!(parse_match_expr("   "), None);
        assert_eq!(parse_match_expr("::"), None);
    }

    #[test]
    fn operators_and_quotes_stay_literal() {
        assert_eq!(
            parse_match_expr("alpha AND beta").as_deref(),
            Some("\"alpha\" \"AND\" \"beta\"*")
        );
        assert_eq!(
            parse_match_expr("say\"hi").as_deref(),
            Some("\"say\"\"hi\"*")
        );
    }
}

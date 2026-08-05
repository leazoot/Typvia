//! CJK-aware text preparation for the unicode61 tokenizer.
//!
//! unicode61 treats a contiguous CJK run as one token, so a substring like
//! "发票" would never match a title "发票抬头". Instead of switching
//! tokenizers — trigram requires 3+ characters per query, breaking
//! two-character Chinese words and short ASCII prefixes — or pulling in a
//! dictionary segmenter dependency, CJK runs are split into one token per
//! character on both the index side and the query side; CJK query terms then
//! match as phrases. Non-CJK text passes through untouched.

/// Han ideograph ranges split for indexing; other scripts are left to
/// unicode61's own word segmentation.
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'      // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}'    // Extension A
        | '\u{F900}'..='\u{FAFF}'    // Compatibility Ideographs
        | '\u{20000}'..='\u{2FA1F}'  // Extensions B..F + supplement
    )
}

/// Rewrites text so every CJK character becomes its own token. Applied to
/// every indexed column and to query terms, keeping both sides symmetric.
pub fn index_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 2);
    let mut prev_cjk = false;
    for c in text.chars() {
        let cjk = is_cjk(c);
        let needs_gap = (cjk && !out.is_empty() && !out.ends_with(char::is_whitespace))
            || (!cjk && prev_cjk && !c.is_whitespace());
        if needs_gap {
            out.push(' ');
        }
        out.push(c);
        prev_cjk = cjk;
    }
    out
}

/// Builds one FTS5 phrase token for a user query term: the term is quoted so
/// FTS5 operators inside user input stay literal, and CJK runs are split the
/// same way as at index time. The query parser (TASK-020) composes these.
pub fn query_phrase(term: &str) -> String {
    let escaped = index_text(term).replace('"', "\"\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_text_passes_through_unchanged() {
        assert_eq!(index_text("docker compose up -d"), "docker compose up -d");
    }

    #[test]
    fn cjk_runs_split_into_single_characters() {
        assert_eq!(index_text("发票抬头"), "发 票 抬 头");
    }

    #[test]
    fn mixed_text_gets_gaps_at_script_boundaries() {
        assert_eq!(index_text("重启nginx服务"), "重 启 nginx 服 务");
        assert_eq!(index_text("deploy 部署 cmd"), "deploy 部 署 cmd");
    }

    #[test]
    fn existing_whitespace_is_not_doubled() {
        assert_eq!(index_text("发票 抬头"), "发 票 抬 头");
    }

    #[test]
    fn query_phrase_quotes_and_splits() {
        assert_eq!(query_phrase("发票"), "\"发 票\"");
        assert_eq!(query_phrase("docker"), "\"docker\"");
    }

    #[test]
    fn query_phrase_neutralizes_fts_operators_and_quotes() {
        assert_eq!(query_phrase("a AND b"), "\"a AND b\"");
        assert_eq!(query_phrase("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}

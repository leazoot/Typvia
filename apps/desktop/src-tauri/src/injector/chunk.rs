//! UTF-16 chunking for CGEvent string posts. A single keyboard event carries
//! roughly 20 UTF-16 units (docs/spikes/injection.md), so typed text is split
//! into bounded chunks without ever cutting a surrogate pair.

/// Splits `text` into substrings of at most `max_units` UTF-16 code units,
/// always on `char` boundaries so surrogate pairs stay whole.
pub(crate) fn chunk_utf16(text: &str, max_units: usize) -> Vec<String> {
    // Invariant: any Unicode scalar value fits in one chunk (2 units max).
    assert!(max_units >= 2, "chunk size must fit a surrogate pair");
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut units = 0;
    for ch in text.chars() {
        let len = ch.len_utf16();
        if units + len > max_units {
            chunks.push(std::mem::take(&mut current));
            units = 0;
        }
        current.push(ch);
        units += len;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::chunk_utf16;

    fn utf16_len(s: &str) -> usize {
        s.encode_utf16().count()
    }

    #[test]
    fn empty_text_yields_no_chunks() {
        assert_eq!(chunk_utf16("", 16), Vec::<String>::new());
    }

    #[test]
    fn short_text_stays_one_chunk() {
        assert_eq!(chunk_utf16("hello", 16), vec!["hello".to_string()]);
    }

    #[test]
    fn ascii_splits_at_exact_unit_boundary() {
        let text = "abcdefghijklmnopqrstuvwxyz";
        let chunks = chunk_utf16(text, 16);
        assert_eq!(
            chunks,
            vec!["abcdefghijklmnop".to_string(), "qrstuvwxyz".to_string()]
        );
    }

    #[test]
    fn every_chunk_respects_the_unit_cap_and_concatenates_back() {
        let text = "Typvia 注入引擎验证:多语言 mixed content, 1234567890";
        let chunks = chunk_utf16(text, 16);
        assert!(chunks.iter().all(|c| utf16_len(c) <= 16));
        assert_eq!(chunks.concat(), text);
    }

    #[test]
    fn surrogate_pairs_are_never_split() {
        // 15 ASCII units followed by an emoji (2 units): the pair must move
        // whole into the next chunk instead of being cut at unit 16.
        let text = format!("{}😀tail", "a".repeat(15));
        let chunks = chunk_utf16(&text, 16);
        assert_eq!(chunks[0], "a".repeat(15));
        assert!(chunks[1].starts_with('😀'));
        assert_eq!(chunks.concat(), text);
        assert!(chunks.iter().all(|c| utf16_len(c) <= 16));
    }

    #[test]
    fn cjk_counts_one_unit_per_character() {
        // 16 CJK chars are exactly 16 units: one full chunk, remainder next.
        let text = "注入引擎验证注入引擎验证注入引擎验证注入";
        let chunks = chunk_utf16(text, 16);
        assert_eq!(utf16_len(&chunks[0]), 16);
        assert_eq!(chunks.concat(), text);
    }
}

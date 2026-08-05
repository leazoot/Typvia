//! High-entropy token detection.
//!
//! A "token" is a maximal run of characters from the base64/hex credential
//! alphabet (`A–Z a–z 0–9 + / = _ -`). A token is reported when all of:
//!
//! 1. It is at least [`MIN_TOKEN_LEN`] characters — shorter runs are too
//!    common in ordinary prose and identifiers.
//! 2. It contains at least one letter and one digit — filters out plain
//!    words, all-digit numbers, and alphabet-like sequences that have high
//!    character diversity but are clearly not machine-generated secrets.
//! 3. Its per-character Shannon entropy exceeds the class threshold:
//!    [`HEX_THRESHOLD`] bits for tokens made only of hex digits (maximum
//!    possible is log2(16) = 4.0) or [`BASE64_THRESHOLD`] bits otherwise
//!    (maximum log2(64) = 6.0). The 3.0/4.5 split follows the widely used
//!    detect-secrets convention: random keys of these alphabets sit well
//!    above it, natural text and identifiers well below.
//!
//! Like every rule in this module the result is advisory; borderline
//! strings (commit hashes, cache keys) may be flagged and the user simply
//! keeps them as normal snippets.

const MIN_TOKEN_LEN: usize = 24;
const HEX_THRESHOLD: f64 = 3.0;
const BASE64_THRESHOLD: f64 = 4.5;

pub(crate) fn contains_high_entropy_token(text: &str) -> bool {
    text.split(|c: char| !is_token_char(c))
        .any(is_high_entropy_token)
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '_' | '-')
}

fn is_high_entropy_token(token: &str) -> bool {
    if token.len() < MIN_TOKEN_LEN {
        return false;
    }
    let has_letter = token.bytes().any(|b| b.is_ascii_alphabetic());
    let has_digit = token.bytes().any(|b| b.is_ascii_digit());
    if !has_letter || !has_digit {
        return false;
    }
    let threshold = if token.bytes().all(|b| b.is_ascii_hexdigit()) {
        HEX_THRESHOLD
    } else {
        BASE64_THRESHOLD
    };
    shannon_entropy_bits(token) > threshold
}

/// Shannon entropy of the token's own character distribution, in bits per
/// character. Tokens are ASCII by construction (see [`is_token_char`]).
fn shannon_entropy_bits(token: &str) -> f64 {
    let mut counts = [0u32; 128];
    for b in token.bytes() {
        counts[usize::from(b) & 0x7F] += 1;
    }
    let len = token.len() as f64;
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = f64::from(c) / len;
            -p * p.log2()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entropy_of_uniform_repetition_is_zero() {
        assert_eq!(shannon_entropy_bits("aaaaaaaa"), 0.0);
    }

    #[test]
    fn flags_random_base64_like_token() {
        assert!(contains_high_entropy_token(
            "key is fake-tXm9Qz4KpLw2Vc8Rb-N5FgH7JdY3TaWqEuZ6MxCoP here"
        ));
    }

    #[test]
    fn flags_random_hex_token_at_lower_threshold() {
        assert!(contains_high_entropy_token(
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
        ));
    }

    #[test]
    fn ignores_prose_and_identifiers() {
        assert!(!contains_high_entropy_token(
            "This is a perfectly ordinary sentence about deployment."
        ));
        assert!(!contains_high_entropy_token(
            "very_long_identifier_name_for_configuration"
        ));
    }

    #[test]
    fn ignores_letter_only_and_digit_only_runs() {
        // High character diversity but no digits: not a machine secret.
        assert!(!contains_high_entropy_token(
            "abcdefghijklmnopqrstuvwxyzABCDEF"
        ));
        assert!(!contains_high_entropy_token("789456123078945612307894"));
    }

    #[test]
    fn ignores_canonical_uuid() {
        // Hex chars plus dashes fall under the base64 threshold.
        assert!(!contains_high_entropy_token(
            "id 550e8400-e29b-41d4-a716-446655440000"
        ));
    }

    #[test]
    fn ignores_tokens_below_minimum_length() {
        assert!(!contains_high_entropy_token("tXm9Qz4KpLw2Vc8RbN5F"));
    }
}

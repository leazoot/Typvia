// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Positive and negative cases for every detection pattern.
//! Every credential-shaped value below is an obvious fake.

use typvia_core::sensitive::{MASK, SensitiveKind, detect, detect_spans, mask_suspected_secrets};

fn detects(text: &str, kind: SensitiveKind) -> bool {
    detect(text).contains(&kind)
}

#[test]
fn pem_private_key_positive_and_negative() {
    for text in [
        "-----BEGIN RSA PRIVATE KEY-----\nFAKEFAKE\n-----END RSA PRIVATE KEY-----",
        "-----BEGIN PRIVATE KEY-----",
        "-----BEGIN OPENSSH PRIVATE KEY-----",
        "-----BEGIN PGP PRIVATE KEY BLOCK-----",
    ] {
        assert!(detects(text, SensitiveKind::PemPrivateKey), "{text}");
    }
    assert!(!detects(
        "-----BEGIN PUBLIC KEY-----",
        SensitiveKind::PemPrivateKey
    ));
    assert!(!detects(
        "-----BEGIN CERTIFICATE-----",
        SensitiveKind::PemPrivateKey
    ));
}

#[test]
fn jwt_positive_and_negative() {
    assert!(detects(
        "token: eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJmYWtlIn0.FAKEsignatureFAKEsignature",
        SensitiveKind::Jwt
    ));
    // A lone base64 header without the two dot-joined parts is not a JWT.
    assert!(!detects("eyJhbGciOiJIUzI1NiJ9", SensitiveKind::Jwt));
    assert!(!detects("code eyJab.eyJcd.ef done", SensitiveKind::Jwt));
}

#[test]
fn bearer_token_positive_and_negative() {
    assert!(detects(
        "Authorization: Bearer FAKE_TOKEN_FAKE_TOKEN_123",
        SensitiveKind::BearerToken
    ));
    assert!(!detects(
        "the bearer of this letter is my friend",
        SensitiveKind::BearerToken
    ));
}

#[test]
fn aws_access_key_positive_and_negative() {
    assert!(detects(
        "aws key AKIAFAKEFAKEFAKEFAKE ok",
        SensitiveKind::AwsAccessKey
    ));
    assert!(detects(
        "temp ASIAFAKEFAKEFAKEFAKE",
        SensitiveKind::AwsAccessKey
    ));
    assert!(!detects(
        "AKIA123 is too short",
        SensitiveKind::AwsAccessKey
    ));
    assert!(!detects(
        "akiafakefakefakefake lowercase",
        SensitiveKind::AwsAccessKey
    ));
}

#[test]
fn github_token_positive_and_negative() {
    assert!(detects(
        "ghp_FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE",
        SensitiveKind::GithubToken
    ));
    assert!(detects(
        "github_pat_FAKEFAKEFAKEFAKEFAKEFAKE",
        SensitiveKind::GithubToken
    ));
    assert!(!detects("ghp_short", SensitiveKind::GithubToken));
}

#[test]
fn api_key_positive_and_negative() {
    assert!(detects(
        "openai sk-FAKEFAKEFAKEFAKEFAKE1234",
        SensitiveKind::ApiKey
    ));
    assert!(detects(
        "slack xoxb-FAKE-FAKE-FAKE-FAKE",
        SensitiveKind::ApiKey
    ));
    assert!(detects(
        "api_key = \"FAKEVALUEFAKEVALUE\"",
        SensitiveKind::ApiKey
    ));
    assert!(detects(
        "CLIENT_SECRET: FAKEVALUEFAKEVALUE",
        SensitiveKind::ApiKey
    ));
    assert!(!detects(
        "the API key is stored in the vault",
        SensitiveKind::ApiKey
    ));
    assert!(!detects("api_key = None", SensitiveKind::ApiKey));
}

#[test]
fn db_connection_string_positive_and_negative() {
    assert!(detects(
        "postgres://admin:fakepass@db.example.com:5432/app",
        SensitiveKind::DbConnectionString
    ));
    assert!(detects(
        "mongodb+srv://user:fakepass@cluster.example.net/db",
        SensitiveKind::DbConnectionString
    ));
    assert!(detects(
        "Server=db.internal;Database=app;User Id=sa;Password=fakepass123;",
        SensitiveKind::DbConnectionString
    ));
    // No credentials in the URL: connection info, not a secret.
    assert!(!detects(
        "postgres://localhost:5432/mydb",
        SensitiveKind::DbConnectionString
    ));
    // A lone password assignment is a PasswordField signal, not a DB one.
    assert!(!detects(
        "password=fakepass123",
        SensitiveKind::DbConnectionString
    ));
}

#[test]
fn cookie_positive_and_negative() {
    assert!(detects(
        "Set-Cookie: session=fakevalue123; HttpOnly",
        SensitiveKind::Cookie
    ));
    assert!(detects(
        "Cookie: theme=dark; other=1",
        SensitiveKind::Cookie
    ));
    assert!(detects("PHPSESSID=fakefake123456", SensitiveKind::Cookie));
    assert!(!detects(
        "I baked a cookie yesterday",
        SensitiveKind::Cookie
    ));
}

#[test]
fn high_entropy_positive_and_negative() {
    assert!(detects(
        "value fake-tXm9Qz4KpLw2Vc8Rb-N5FgH7JdY3TaWqEuZ6MxCoP",
        SensitiveKind::HighEntropyString
    ));
    assert!(!detects(
        "A long note describing the weekly deployment procedure in detail.",
        SensitiveKind::HighEntropyString
    ));
}

#[test]
fn password_field_positive_and_negative() {
    assert!(detects(
        "password: fakehunter2",
        SensitiveKind::PasswordField
    ));
    assert!(detects("PASSWD=fakevalue", SensitiveKind::PasswordField));
    assert!(detects("密码:假密码123", SensitiveKind::PasswordField));
    assert!(!detects(
        "Remember to rotate your password regularly",
        SensitiveKind::PasswordField
    ));
}

#[test]
fn plain_text_yields_no_findings() {
    assert!(
        detect("Meeting notes: discuss the roadmap, then lunch at noon. 会议记录。").is_empty()
    );
}

#[test]
fn multiple_patterns_report_in_stable_order() {
    let text = "key AKIAFAKEFAKEFAKEFAKE and password: fakehunter2";
    assert_eq!(
        detect(text),
        vec![SensitiveKind::AwsAccessKey, SensitiveKind::PasswordField]
    );
}

#[test]
fn spans_cover_exactly_the_matched_bytes() {
    let text = "key AKIAFAKEFAKEFAKEFAKE ok";
    let spans = detect_spans(text);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].kind, SensitiveKind::AwsAccessKey);
    assert_eq!(&text[spans[0].start..spans[0].end], "AKIAFAKEFAKEFAKEFAKE");
}

#[test]
fn spans_report_every_occurrence_sorted_by_offset() {
    let text = "a AKIAFAKEFAKEFAKEFAKE b password: fakehunter2 c AKIAFAKEFAKEFAKEFAKE";
    let spans = detect_spans(text);
    assert_eq!(spans.len(), 3);
    assert!(spans.windows(2).all(|w| w[0].start <= w[1].start));
    assert_eq!(
        spans
            .iter()
            .filter(|s| s.kind == SensitiveKind::AwsAccessKey)
            .count(),
        2
    );
}

#[test]
fn spans_are_empty_for_plain_text() {
    assert!(detect_spans("Lunch at noon, then the roadmap review. 会议记录。").is_empty());
}

#[test]
fn masking_removes_matches_and_survives_a_rescan() {
    let text = "deploy key AKIAFAKEFAKEFAKEFAKE and 密码:假密码123 for the demo box";
    // Canary: prove the scanner sees the input before asserting removal.
    assert!(!detect(text).is_empty());

    let outcome = mask_suspected_secrets(text);
    assert!(detect(&outcome.masked).is_empty());
    assert!(!outcome.masked.contains("AKIAFAKEFAKEFAKEFAKE"));
    assert!(!outcome.masked.contains("假密码123"));
    assert!(outcome.masked.contains(MASK));
    assert!(outcome.masked.starts_with("deploy key "));
    assert!(outcome.masked.ends_with(" for the demo box"));
    assert_eq!(
        outcome.kinds,
        vec![SensitiveKind::AwsAccessKey, SensitiveKind::PasswordField]
    );
}

#[test]
fn masking_clean_text_changes_nothing() {
    let outcome = mask_suspected_secrets("An ordinary snippet about lunch plans.");
    assert_eq!(outcome.masked, "An ordinary snippet about lunch plans.");
    assert!(outcome.kinds.is_empty());
}

#[test]
fn masking_handles_overlapping_kinds() {
    // The assignment form is both an ApiKey match and (value) a candidate
    // high-entropy token; overlapping ranges must not duplicate content.
    let text = "api_key = fake-tXm9Qz4KpLw2Vc8Rb-N5FgH7JdY3TaWqEuZ6MxCoP";
    assert!(!detect(text).is_empty());
    let outcome = mask_suspected_secrets(text);
    assert!(detect(&outcome.masked).is_empty());
    assert!(!outcome.masked.contains("tXm9Qz4K"));
}

#[test]
fn kind_codes_are_stable() {
    assert_eq!(SensitiveKind::PemPrivateKey.as_str(), "pem_private_key");
    assert_eq!(
        SensitiveKind::HighEntropyString.as_str(),
        "high_entropy_string"
    );
}

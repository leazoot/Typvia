//! Regex rules for the fixed-format sensitive-content patterns. The
//! high-entropy detector lives in `entropy`; everything else is matched here.

use std::sync::LazyLock;

use regex::Regex;

use crate::sensitive::SensitiveKind;

// Detection is advisory (the user can always keep a match as a normal
// snippet), so patterns favor recall over precision: a value that merely
// looks like a credential is worth a hint.

static PEM_PRIVATE_KEY: LazyLock<Regex> =
    LazyLock::new(|| compiled(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY( BLOCK)?-----"));

// JWS compact form: base64url JSON header and payload both start with
// `eyJ` (base64 of `{"`), then a signature part.
static JWT: LazyLock<Regex> =
    LazyLock::new(|| compiled(r"\beyJ[A-Za-z0-9_-]{4,}\.eyJ[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{10,}"));

static BEARER_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| compiled(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=_-]{16,}"));

// Long-term (AKIA) and temporary (ASIA) access key IDs.
static AWS_ACCESS_KEY: LazyLock<Regex> = LazyLock::new(|| compiled(r"\b(AKIA|ASIA)[0-9A-Z]{16}\b"));

// Classic (ghp_/gho_/ghu_/ghs_/ghr_) and fine-grained (github_pat_) tokens.
static GITHUB_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| compiled(r"\b(gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,})"));

// Well-known vendor prefixes (OpenAI, Slack, Google) — recognizable without
// any surrounding context.
static API_KEY_PREFIXED: LazyLock<Regex> = LazyLock::new(|| {
    compiled(r"\b(sk-[A-Za-z0-9_-]{20,}|xox[baprs]-[A-Za-z0-9-]{10,}|AIza[0-9A-Za-z_-]{35})")
});

// Generic `some_api_key = <long value>` assignments.
static API_KEY_ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    compiled(
        r#"(?i)\b(api[_-]?key|access[_-]?key|secret[_-]?key|client[_-]?secret)["']?\s*[:=]\s*["']?[A-Za-z0-9_\-./+]{16,}"#,
    )
});

// Credentials embedded in a database URL: `scheme://user:password@host`.
static DB_URL_WITH_CREDENTIALS: LazyLock<Regex> = LazyLock::new(|| {
    compiled(
        r"(?i)\b(postgres(ql)?|mysql|mariadb|mongodb(\+srv)?|redis|rediss|amqp|mssql|sqlserver)://[^\s:@/]+:[^\s@/]+@",
    )
});

// ADO/JDBC-style key-value connection strings; both halves must be present
// so a lone `password=` stays a PasswordField signal, not a DB one.
static DB_KV_HOST: LazyLock<Regex> =
    LazyLock::new(|| compiled(r"(?i)\b(server|data source|host)\s*="));
static DB_KV_PASSWORD: LazyLock<Regex> =
    LazyLock::new(|| compiled(r"(?i)\b(password|pwd)\s*=\s*[^;\s]+"));

static COOKIE_HEADER: LazyLock<Regex> =
    LazyLock::new(|| compiled(r"(?im)^\s*(set-cookie|cookie)\s*:\s*\S+"));

static COOKIE_SESSION_PAIR: LazyLock<Regex> = LazyLock::new(|| {
    compiled(
        r"(?i)\b(jsessionid|phpsessid|sessionid|session_id|csrf[_-]?token|xsrf[_-]?token)\s*=\s*[A-Za-z0-9%+/=_-]{8,}",
    )
});

static PASSWORD_FIELD: LazyLock<Regex> =
    LazyLock::new(|| compiled(r"(?i)\b(password|passwd|pwd|passphrase)\s*[:=]\s*\S+"));

// `\b` does not delimit CJK, so the Chinese label gets its own pattern
// (full-width and ASCII separators both occur in pasted notes).
static PASSWORD_FIELD_CN: LazyLock<Regex> = LazyLock::new(|| compiled(r"密码\s*[::=]\s*\S+"));

/// Byte ranges every pattern of `kind` matches in `text` (for masking).
/// Kinds whose match needs two cooperating patterns (`DbConnectionString`
/// key-value form) report only the credential-bearing part — the host half
/// is not a secret.
pub(crate) fn find_spans(kind: SensitiveKind, text: &str, spans: &mut Vec<(usize, usize)>) {
    let mut push_all = |re: &Regex| {
        spans.extend(re.find_iter(text).map(|m| (m.start(), m.end())));
    };
    match kind {
        SensitiveKind::PemPrivateKey => push_all(&PEM_PRIVATE_KEY),
        SensitiveKind::Jwt => push_all(&JWT),
        SensitiveKind::BearerToken => push_all(&BEARER_TOKEN),
        SensitiveKind::AwsAccessKey => push_all(&AWS_ACCESS_KEY),
        SensitiveKind::GithubToken => push_all(&GITHUB_TOKEN),
        SensitiveKind::ApiKey => {
            push_all(&API_KEY_PREFIXED);
            push_all(&API_KEY_ASSIGNMENT);
        }
        SensitiveKind::DbConnectionString => {
            push_all(&DB_URL_WITH_CREDENTIALS);
            if DB_KV_HOST.is_match(text) {
                push_all(&DB_KV_PASSWORD);
            }
        }
        SensitiveKind::Cookie => {
            push_all(&COOKIE_HEADER);
            push_all(&COOKIE_SESSION_PAIR);
        }
        SensitiveKind::PasswordField => {
            push_all(&PASSWORD_FIELD);
            push_all(&PASSWORD_FIELD_CN);
        }
        SensitiveKind::HighEntropyString => {}
    }
}

pub(crate) fn matches(kind: SensitiveKind, text: &str) -> bool {
    match kind {
        SensitiveKind::PemPrivateKey => PEM_PRIVATE_KEY.is_match(text),
        SensitiveKind::Jwt => JWT.is_match(text),
        SensitiveKind::BearerToken => BEARER_TOKEN.is_match(text),
        SensitiveKind::AwsAccessKey => AWS_ACCESS_KEY.is_match(text),
        SensitiveKind::GithubToken => GITHUB_TOKEN.is_match(text),
        SensitiveKind::ApiKey => {
            API_KEY_PREFIXED.is_match(text) || API_KEY_ASSIGNMENT.is_match(text)
        }
        SensitiveKind::DbConnectionString => {
            DB_URL_WITH_CREDENTIALS.is_match(text)
                || (DB_KV_HOST.is_match(text) && DB_KV_PASSWORD.is_match(text))
        }
        SensitiveKind::Cookie => COOKIE_HEADER.is_match(text) || COOKIE_SESSION_PAIR.is_match(text),
        SensitiveKind::PasswordField => {
            PASSWORD_FIELD.is_match(text) || PASSWORD_FIELD_CN.is_match(text)
        }
        SensitiveKind::HighEntropyString => false,
    }
}

// Invariant: only called with the fixed pattern literals above, each of
// which is exercised by the tests in tests/sensitive_detection.rs.
#[allow(clippy::expect_used)]
fn compiled(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static detection pattern must compile")
}

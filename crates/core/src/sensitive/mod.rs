//! Offline sensitive-content detection (docs/PRD.md §12.10).
//!
//! Pure rule engine: [`detect`] scans a piece of text and reports which
//! suspicious patterns it saw. The result is advisory only — the save flow
//! shows a hint ("this may contain sensitive information") and the user
//! decides; nothing here blocks a save or changes data. Findings carry the
//! pattern kind and never the matched text, so they are safe to pass across
//! layers and to log.

mod entropy;
mod rules;

/// One category of suspicious content, mirroring the PRD §12.10 list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitiveKind {
    PemPrivateKey,
    Jwt,
    BearerToken,
    AwsAccessKey,
    GithubToken,
    ApiKey,
    DbConnectionString,
    Cookie,
    HighEntropyString,
    PasswordField,
}

impl SensitiveKind {
    /// Every kind, in the stable order results are reported in.
    pub const ALL: [SensitiveKind; 10] = [
        SensitiveKind::PemPrivateKey,
        SensitiveKind::Jwt,
        SensitiveKind::BearerToken,
        SensitiveKind::AwsAccessKey,
        SensitiveKind::GithubToken,
        SensitiveKind::ApiKey,
        SensitiveKind::DbConnectionString,
        SensitiveKind::Cookie,
        SensitiveKind::HighEntropyString,
        SensitiveKind::PasswordField,
    ];

    /// Stable machine-readable code (IPC / persistence boundary).
    pub fn as_str(self) -> &'static str {
        match self {
            SensitiveKind::PemPrivateKey => "pem_private_key",
            SensitiveKind::Jwt => "jwt",
            SensitiveKind::BearerToken => "bearer_token",
            SensitiveKind::AwsAccessKey => "aws_access_key",
            SensitiveKind::GithubToken => "github_token",
            SensitiveKind::ApiKey => "api_key",
            SensitiveKind::DbConnectionString => "db_connection_string",
            SensitiveKind::Cookie => "cookie",
            SensitiveKind::HighEntropyString => "high_entropy_string",
            SensitiveKind::PasswordField => "password_field",
        }
    }
}

/// Scans `text` and returns every pattern kind it appears to contain, in
/// [`SensitiveKind::ALL`] order, each at most once. Empty means nothing
/// suspicious was found. Purely local and offline; no I/O.
pub fn detect(text: &str) -> Vec<SensitiveKind> {
    SensitiveKind::ALL
        .into_iter()
        .filter(|&kind| match kind {
            SensitiveKind::HighEntropyString => entropy::contains_high_entropy_token(text),
            _ => rules::matches(kind, text),
        })
        .collect()
}

//! Offline sensitive-content detection.
//!
//! Pure rule engine: [`detect`] scans a piece of text and reports which
//! suspicious patterns it saw. The result is advisory only — the save flow
//! shows a hint ("this may contain sensitive information") and the user
//! decides; nothing here blocks a save or changes data. Findings carry the
//! pattern kind and never the matched text, so they are safe to pass across
//! layers and to log.

mod entropy;
mod rules;

/// One category of suspicious content.
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

/// One suspicious match with its byte range in the scanned text. Carries
/// offsets only, never the matched text — safe to pass across layers, like
/// [`detect`] results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SensitiveSpan {
    pub kind: SensitiveKind,
    /// Byte offset of the match start (inclusive).
    pub start: usize,
    /// Byte offset of the match end (exclusive).
    pub end: usize,
}

/// Like [`detect`], but reports every match's byte range, sorted by start
/// offset. Ranges from different kinds may overlap.
pub fn detect_spans(text: &str) -> Vec<SensitiveSpan> {
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut spans = Vec::new();
    for kind in SensitiveKind::ALL {
        ranges.clear();
        match kind {
            SensitiveKind::HighEntropyString => ranges.extend(entropy::high_entropy_spans(text)),
            _ => rules::find_spans(kind, text, &mut ranges),
        }
        spans.extend(
            ranges
                .iter()
                .map(|&(start, end)| SensitiveSpan { kind, start, end }),
        );
    }
    spans.sort_by_key(|s| (s.start, s.end));
    spans
}

/// Replacement written over each masked range. Deliberately outside every
/// credential alphabet so masking cannot manufacture a new match.
pub const MASK: &str = "[secret removed]";

/// The result of [`mask_suspected_secrets`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskOutcome {
    /// The text with every suspicious range replaced by [`MASK`].
    pub masked: String,
    /// Which kinds were masked, in [`SensitiveKind::ALL`] order.
    pub kinds: Vec<SensitiveKind>,
}

/// How many detect-and-replace rounds masking runs before giving up on
/// convergence. One round almost always suffices; the caller must still
/// verify the outcome with [`detect`] and refuse on residual hits
/// (fail-closed), so a non-converged result is safe.
const MAX_MASK_ROUNDS: usize = 5;

/// Replaces every suspicious range in `text` with [`MASK`] and re-scans
/// until clean: the mask_secrets permission scope strips suspected secrets
/// before egress. Callers MUST re-run
/// [`detect`] on the result and refuse to send on any remaining hit.
pub fn mask_suspected_secrets(text: &str) -> MaskOutcome {
    let mut masked = text.to_string();
    let mut seen = [false; SensitiveKind::ALL.len()];
    for _ in 0..MAX_MASK_ROUNDS {
        let spans = detect_spans(&masked);
        if spans.is_empty() {
            break;
        }
        let mut next = String::with_capacity(masked.len());
        let mut cursor = 0;
        for span in &spans {
            let index = SensitiveKind::ALL
                .iter()
                .position(|&k| k == span.kind)
                .unwrap_or(0);
            seen[index] = true;
            // Overlapping ranges collapse into the already-masked region.
            if span.start < cursor {
                cursor = cursor.max(span.end);
                continue;
            }
            next.push_str(&masked[cursor..span.start]);
            next.push_str(MASK);
            cursor = span.end;
        }
        next.push_str(&masked[cursor..]);
        masked = next;
    }
    let kinds = SensitiveKind::ALL
        .into_iter()
        .enumerate()
        .filter_map(|(i, kind)| seen[i].then_some(kind))
        .collect();
    MaskOutcome { masked, kinds }
}

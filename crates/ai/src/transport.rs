//! The HTTP seam ([`AiTransport`]) between the wire client and the
//! network, so unit tests substitute a double and never touch the network.
//! The API key crosses this boundary as a typed [`ApiKey`]
//! field — implementations may place it only in the `Authorization`
//! header.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::credentials::ApiKey;
use crate::error::AiError;

/// Cooperative cancellation handle shared between the caller and an
/// in-flight request. Cancellation takes effect at the next checkpoint
/// (before send, between response chunks); the overall wait is always
/// bounded by the request timeout.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

/// One request as the wire client hands it to the transport.
pub struct TransportRequest<'a> {
    pub method: HttpMethod,
    pub url: String,
    /// Placed in the `Authorization` header (`Bearer <key>`) and nowhere
    /// else.
    pub api_key: Option<&'a ApiKey>,
    /// JSON body for `Post`.
    pub body: Option<&'a [u8]>,
    pub timeout: Duration,
}

pub struct TransportResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl std::fmt::Debug for TransportResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Response bodies are provider/user content and never reach Debug.
        f.debug_struct("TransportResponse")
            .field("status", &self.status)
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// Transport-level failures, before any wire-shape interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportFailure {
    /// The URL violates the crate's TLS/loopback policy. Kept as a
    /// transport guard even though configs validate it, so no future
    /// caller can bypass the rule.
    InsecureUrl,
    Cancelled,
    Timeout,
    /// DNS, connect or TLS failure before a response arrived.
    Unreachable,
    /// The connection broke mid-request.
    Interrupted,
    ResponseTooLarge,
}

impl From<TransportFailure> for AiError {
    fn from(failure: TransportFailure) -> Self {
        match failure {
            TransportFailure::InsecureUrl => AiError::InvalidConfig {
                field: "base_url",
                rule: "must be https://, or http:// toward loopback",
            },
            TransportFailure::Cancelled => AiError::Cancelled,
            TransportFailure::Timeout => AiError::Timeout,
            TransportFailure::Unreachable => AiError::Unreachable,
            TransportFailure::Interrupted => AiError::Transport,
            TransportFailure::ResponseTooLarge => AiError::InvalidResponse,
        }
    }
}

/// Executes one HTTP exchange. Implementations must honor `timeout`,
/// consult `cancel` at their checkpoints, verify TLS unconditionally and
/// log nothing.
pub trait AiTransport {
    fn execute(
        &self,
        request: &TransportRequest<'_>,
        cancel: &CancelToken,
    ) -> Result<TransportResponse, TransportFailure>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cloned_token_shares_cancellation_state() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        assert!(clone.is_cancelled());
    }

    #[test]
    fn transport_failures_map_onto_the_error_taxonomy() {
        assert_eq!(
            AiError::from(TransportFailure::Cancelled),
            AiError::Cancelled
        );
        assert_eq!(AiError::from(TransportFailure::Timeout), AiError::Timeout);
        assert_eq!(
            AiError::from(TransportFailure::Unreachable),
            AiError::Unreachable
        );
        assert_eq!(
            AiError::from(TransportFailure::Interrupted),
            AiError::Transport
        );
        assert_eq!(
            AiError::from(TransportFailure::ResponseTooLarge),
            AiError::InvalidResponse
        );
        assert!(matches!(
            AiError::from(TransportFailure::InsecureUrl),
            AiError::InvalidConfig {
                field: "base_url",
                ..
            }
        ));
    }
}

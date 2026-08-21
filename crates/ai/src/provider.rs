// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The [`AiProvider`] trait business code programs against — external
//! services are only ever reached behind traits — and the single
//! OpenAI-compatible client that serves all four provider kinds.
//!
//! Prompt and completion content is user content: `Debug` renderings carry
//! lengths only, and serialized request bytes wipe on drop.

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use typvia_core::model::AiRequestClass;
use zeroize::Zeroizing;

use crate::config::{ProviderConfig, ProviderKindExt as _};
use crate::credentials::ApiKey;
use crate::egress::{self, EgressEntry, EgressLog};
use crate::error::AiError;
use crate::transport::{AiTransport, CancelToken, HttpMethod, TransportRequest};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl fmt::Debug for ChatMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Message content is user content and never reaches Debug.
        f.debug_struct("ChatMessage")
            .field("role", &self.role)
            .field("content_len", &self.content.len())
            .finish()
    }
}

/// One completion request, provider-agnostic.
#[derive(Clone, PartialEq)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

impl ChatRequest {
    pub fn validate(&self) -> Result<(), AiError> {
        if self.messages.is_empty() {
            return Err(AiError::InvalidRequest {
                rule: "must contain at least one message",
            });
        }
        if let Some(t) = self.temperature
            && !(0.0..=2.0).contains(&t)
        {
            return Err(AiError::InvalidRequest {
                rule: "temperature must be between 0 and 2",
            });
        }
        if self.max_tokens == Some(0) {
            return Err(AiError::InvalidRequest {
                rule: "max_tokens must be positive",
            });
        }
        Ok(())
    }
}

impl fmt::Debug for ChatRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatRequest")
            .field("messages", &self.messages)
            .field("temperature", &self.temperature)
            .field("max_tokens", &self.max_tokens)
            .finish()
    }
}

/// The provider's answer.
#[derive(Clone, PartialEq, Eq)]
pub struct ChatOutcome {
    pub content: String,
}

impl fmt::Debug for ChatOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatOutcome")
            .field("content_len", &self.content.len())
            .finish()
    }
}

/// Result of a connectivity check. `model_available` is `None` when the
/// endpoint is reachable but does not enumerate models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connectivity {
    pub model_available: Option<bool>,
}

/// What business code sees of an AI provider. No concrete AI feature
/// (organizing, extraction, actions) lives at this level.
pub trait AiProvider {
    fn complete(&self, request: &ChatRequest, cancel: &CancelToken)
    -> Result<ChatOutcome, AiError>;

    /// Cheap reachability probe for the settings UI: `GET /models`,
    /// reporting whether the configured model is offered when the server
    /// enumerates models.
    fn check_connectivity(&self, cancel: &CancelToken) -> Result<Connectivity, AiError>;
}

/// The OpenAI-compatible client behind all four [`ProviderKind`]s
/// (`crate::config`): kind differences are entirely configuration.
///
/// The egress log is a mandatory constructor argument, not an option — the
/// only path to the transport runs through the sensitive scan and the log
/// write, and no configuration exists that skips either.
pub struct OpenAiCompatProvider<T: AiTransport> {
    config: ProviderConfig,
    api_key: Option<ApiKey>,
    transport: T,
    egress_log: Arc<dyn EgressLog>,
}

impl<T: AiTransport> fmt::Debug for OpenAiCompatProvider<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiCompatProvider")
            .field("config", &self.config)
            .field("has_api_key", &self.api_key.is_some())
            .finish()
    }
}

impl<T: AiTransport> OpenAiCompatProvider<T> {
    /// Validates the config and the key requirement up front so every
    /// later call runs against a known-good configuration.
    pub fn new(
        config: ProviderConfig,
        api_key: Option<ApiKey>,
        transport: T,
        egress_log: Arc<dyn EgressLog>,
    ) -> Result<Self, AiError> {
        config.validate()?;
        if config.kind.requires_api_key() && api_key.is_none() {
            return Err(AiError::MissingApiKey);
        }
        Ok(Self {
            config,
            api_key,
            transport,
            egress_log,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url, path)
    }

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.config.timeout_ms)
    }

    /// Records the imminent send, refusing to proceed when the log cannot
    /// be written (fail-closed: no log entry, no egress).
    fn record_egress(
        &self,
        request_class: AiRequestClass,
        request_bytes: u64,
    ) -> Result<(), AiError> {
        self.egress_log.record(&EgressEntry {
            provider_id: self.config.id.clone(),
            request_class,
            request_bytes,
        })?;
        Ok(())
    }
}

impl<T: AiTransport> AiProvider for OpenAiCompatProvider<T> {
    fn complete(
        &self,
        request: &ChatRequest,
        cancel: &CancelToken,
    ) -> Result<ChatOutcome, AiError> {
        request.validate()?;
        // Egress gate: no message content reaches serialization,
        // the log or the wire while it looks like it carries a secret.
        for message in &request.messages {
            egress::scan_outbound(&message.content)?;
        }
        let wire = WireChatRequest {
            model: &self.config.model,
            messages: request
                .messages
                .iter()
                .map(|m| WireMessage {
                    role: m.role,
                    content: &m.content,
                })
                .collect(),
            stream: false,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
        };
        // Serialized prompts are user content; the buffer wipes on drop.
        let body =
            Zeroizing::new(
                serde_json::to_vec(&wire).map_err(|_| AiError::InvalidRequest {
                    rule: "message content must be serializable",
                })?,
            );
        self.record_egress(AiRequestClass::Completion, body.len() as u64)?;
        let response = self.transport.execute(
            &TransportRequest {
                method: HttpMethod::Post,
                url: self.url("/chat/completions"),
                api_key: self.api_key.as_ref(),
                body: Some(&body),
                timeout: self.timeout(),
            },
            cancel,
        )?;
        if !(200..300).contains(&response.status) {
            return Err(classify_status(response.status));
        }
        let parsed: WireChatResponse =
            serde_json::from_slice(&response.body).map_err(|_| AiError::InvalidResponse)?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or(AiError::InvalidResponse)?;
        Ok(ChatOutcome { content })
    }

    fn check_connectivity(&self, cancel: &CancelToken) -> Result<Connectivity, AiError> {
        self.record_egress(AiRequestClass::Connectivity, 0)?;
        let response = self.transport.execute(
            &TransportRequest {
                method: HttpMethod::Get,
                url: self.url("/models"),
                api_key: self.api_key.as_ref(),
                body: None,
                timeout: self.timeout(),
            },
            cancel,
        )?;
        match response.status {
            // Reachable but no model listing (some compatible servers do
            // not implement `/models`) — still a successful probe.
            404 | 501 => {
                return Ok(Connectivity {
                    model_available: None,
                });
            }
            status if !(200..300).contains(&status) => return Err(classify_status(status)),
            _ => {}
        }
        let listed = serde_json::from_slice::<WireModelList>(&response.body)
            .ok()
            .map(|list| list.data.iter().any(|m| m.id == self.config.model));
        Ok(Connectivity {
            model_available: listed,
        })
    }
}

/// Maps non-2xx statuses onto the taxonomy. 404 on `/chat/completions` is
/// how OpenAI-compatible servers report an unknown model.
fn classify_status(status: u16) -> AiError {
    match status {
        401 | 403 => AiError::AuthRejected,
        404 => AiError::ModelNotFound,
        429 => AiError::RateLimited,
        400..=499 => AiError::ProviderRejected { status },
        _ => AiError::ProviderFailure { status },
    }
}

#[derive(Serialize)]
struct WireChatRequest<'a> {
    model: &'a str,
    messages: Vec<WireMessage<'a>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Serialize)]
struct WireMessage<'a> {
    role: Role,
    content: &'a str,
}

#[derive(Deserialize)]
struct WireChatResponse {
    choices: Vec<WireChoice>,
}

#[derive(Deserialize)]
struct WireChoice {
    message: WireResponseMessage,
}

#[derive(Deserialize)]
struct WireResponseMessage {
    content: String,
}

#[derive(Deserialize)]
struct WireModelList {
    data: Vec<WireModel>,
}

#[derive(Deserialize)]
struct WireModel {
    id: String,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::cell::RefCell;
    use std::sync::Mutex;
    use std::time::Duration;

    use super::*;
    use crate::config::ProviderKind;
    use crate::egress::EgressLogError;
    use crate::transport::{TransportFailure, TransportResponse};

    /// Sink double that keeps every recorded entry for assertions.
    #[derive(Default)]
    struct RecordingLog {
        entries: Mutex<Vec<EgressEntry>>,
    }

    impl EgressLog for RecordingLog {
        fn record(&self, entry: &EgressEntry) -> Result<(), EgressLogError> {
            self.entries.lock().unwrap().push(entry.clone());
            Ok(())
        }
    }

    /// Sink double whose writes always fail (fail-closed proof).
    struct FailingLog;

    impl EgressLog for FailingLog {
        fn record(&self, _: &EgressEntry) -> Result<(), EgressLogError> {
            Err(EgressLogError)
        }
    }

    fn noop_log() -> Arc<RecordingLog> {
        Arc::new(RecordingLog::default())
    }

    /// Recorded copy of one request as it crossed the transport seam.
    struct SeenRequest {
        method: HttpMethod,
        url: String,
        bearer: Option<String>,
        body: Option<Vec<u8>>,
        timeout: Duration,
    }

    /// Transport double: returns scripted responses and records requests.
    struct FakeTransport {
        script: RefCell<Vec<Result<(u16, &'static str), TransportFailure>>>,
        seen: RefCell<Vec<SeenRequest>>,
    }

    impl FakeTransport {
        fn respond(status: u16, body: &'static str) -> Self {
            Self {
                script: RefCell::new(vec![Ok((status, body))]),
                seen: RefCell::new(Vec::new()),
            }
        }

        fn fail(failure: TransportFailure) -> Self {
            Self {
                script: RefCell::new(vec![Err(failure)]),
                seen: RefCell::new(Vec::new()),
            }
        }
    }

    impl AiTransport for FakeTransport {
        fn execute(
            &self,
            request: &TransportRequest<'_>,
            cancel: &CancelToken,
        ) -> Result<TransportResponse, TransportFailure> {
            if cancel.is_cancelled() {
                return Err(TransportFailure::Cancelled);
            }
            self.seen.borrow_mut().push(SeenRequest {
                method: request.method,
                url: request.url.clone(),
                bearer: request.api_key.map(|k| k.expose().to_string()),
                body: request.body.map(<[u8]>::to_vec),
                timeout: request.timeout,
            });
            let (status, body) = self.script.borrow_mut().remove(0)?;
            Ok(TransportResponse {
                status,
                body: body.as_bytes().to_vec(),
            })
        }
    }

    fn config(kind: ProviderKind) -> ProviderConfig {
        let base = match kind {
            ProviderKind::OpenAiCompatible => Some("https://api.example.com/v1"),
            ProviderKind::CustomBaseUrl => Some("https://gateway.example.com/openai/v1"),
            _ => None,
        };
        ProviderConfig::new("p1", kind, base, "test-model").unwrap()
    }

    fn chat() -> ChatRequest {
        ChatRequest {
            messages: vec![ChatMessage {
                role: Role::User,
                content: "organize this snippet".into(),
            }],
            temperature: Some(0.2),
            max_tokens: Some(256),
        }
    }

    const COMPLETION: &str =
        r#"{"choices":[{"message":{"role":"assistant","content":"the answer"}}]}"#;

    #[test]
    fn each_provider_kind_posts_the_same_wire_format_to_its_base_url() {
        for kind in [
            ProviderKind::Ollama,
            ProviderKind::LmStudio,
            ProviderKind::OpenAiCompatible,
            ProviderKind::CustomBaseUrl,
        ] {
            let key = ApiKey::new("sk-FAKE-key").unwrap();
            let transport = FakeTransport::respond(200, COMPLETION);
            let config = config(kind);
            let expected_url = format!("{}/chat/completions", config.base_url);
            let provider =
                OpenAiCompatProvider::new(config, Some(key), transport, noop_log()).unwrap();
            let outcome = provider.complete(&chat(), &CancelToken::new()).unwrap();
            assert_eq!(outcome.content, "the answer", "{kind:?}");

            let seen = provider.transport.seen.borrow();
            assert_eq!(seen.len(), 1);
            assert_eq!(seen[0].method, HttpMethod::Post);
            assert_eq!(seen[0].url, expected_url, "{kind:?}");
            let body: serde_json::Value =
                serde_json::from_slice(seen[0].body.as_deref().unwrap()).unwrap();
            assert_eq!(body["model"], "test-model");
            assert_eq!(body["stream"], false);
            assert_eq!(body["messages"][0]["role"], "user");
            assert_eq!(body["messages"][0]["content"], "organize this snippet");
            assert_eq!(body["max_tokens"], 256);
        }
    }

    #[test]
    fn the_key_crosses_the_seam_only_as_the_typed_credential_field() {
        let key = ApiKey::new("sk-FAKE-seam-canary").unwrap();
        let transport = FakeTransport::respond(200, COMPLETION);
        let provider = OpenAiCompatProvider::new(
            config(ProviderKind::OpenAiCompatible),
            Some(key),
            transport,
            noop_log(),
        )
        .unwrap();
        provider.complete(&chat(), &CancelToken::new()).unwrap();
        let seen = provider.transport.seen.borrow();
        assert_eq!(seen[0].bearer.as_deref(), Some("sk-FAKE-seam-canary"));
        // Red line: the key never leaks into the URL or the JSON body.
        assert!(!seen[0].url.contains("sk-FAKE-seam-canary"));
        let body = String::from_utf8(seen[0].body.clone().unwrap()).unwrap();
        assert!(!body.contains("sk-FAKE-seam-canary"));
    }

    #[test]
    fn local_kinds_work_without_a_key_and_send_none() {
        let transport = FakeTransport::respond(200, COMPLETION);
        let provider =
            OpenAiCompatProvider::new(config(ProviderKind::Ollama), None, transport, noop_log())
                .unwrap();
        provider.complete(&chat(), &CancelToken::new()).unwrap();
        assert!(provider.transport.seen.borrow()[0].bearer.is_none());
    }

    #[test]
    fn a_hosted_kind_without_a_key_is_refused_before_any_call() {
        let transport = FakeTransport::respond(200, COMPLETION);
        let refused = OpenAiCompatProvider::new(
            config(ProviderKind::OpenAiCompatible),
            None,
            transport,
            noop_log(),
        );
        assert!(matches!(refused, Err(AiError::MissingApiKey)));
    }

    #[test]
    fn the_configured_timeout_reaches_the_transport() {
        let transport = FakeTransport::respond(200, COMPLETION);
        let mut config = config(ProviderKind::Ollama);
        config.timeout_ms = 7_000;
        let provider = OpenAiCompatProvider::new(config, None, transport, noop_log()).unwrap();
        provider.complete(&chat(), &CancelToken::new()).unwrap();
        assert_eq!(
            provider.transport.seen.borrow()[0].timeout,
            Duration::from_millis(7_000)
        );
    }

    #[test]
    fn provider_statuses_map_onto_the_error_taxonomy() {
        for (status, expected) in [
            (401, AiError::AuthRejected),
            (403, AiError::AuthRejected),
            (404, AiError::ModelNotFound),
            (429, AiError::RateLimited),
            (422, AiError::ProviderRejected { status: 422 }),
            (500, AiError::ProviderFailure { status: 500 }),
            (503, AiError::ProviderFailure { status: 503 }),
        ] {
            let transport = FakeTransport::respond(status, r#"{"error":"details"}"#);
            let provider = OpenAiCompatProvider::new(
                config(ProviderKind::Ollama),
                None,
                transport,
                noop_log(),
            )
            .unwrap();
            let error = provider.complete(&chat(), &CancelToken::new()).unwrap_err();
            assert_eq!(error, expected, "status {status}");
        }
    }

    #[test]
    fn transport_failures_and_cancellation_surface_classified() {
        for (failure, expected) in [
            (TransportFailure::Timeout, AiError::Timeout),
            (TransportFailure::Unreachable, AiError::Unreachable),
            (TransportFailure::Interrupted, AiError::Transport),
        ] {
            let transport = FakeTransport::fail(failure);
            let provider = OpenAiCompatProvider::new(
                config(ProviderKind::Ollama),
                None,
                transport,
                noop_log(),
            )
            .unwrap();
            let error = provider.complete(&chat(), &CancelToken::new()).unwrap_err();
            assert_eq!(error, expected);
        }

        let cancel = CancelToken::new();
        cancel.cancel();
        let transport = FakeTransport::respond(200, COMPLETION);
        let provider =
            OpenAiCompatProvider::new(config(ProviderKind::Ollama), None, transport, noop_log())
                .unwrap();
        let error = provider.complete(&chat(), &cancel).unwrap_err();
        assert_eq!(error, AiError::Cancelled);
        // The cancelled request never crossed the seam.
        assert!(provider.transport.seen.borrow().is_empty());
    }

    #[test]
    fn malformed_or_empty_answers_are_invalid_response() {
        for body in ["not json", r#"{"choices":[]}"#, r#"{"unexpected":true}"#] {
            let transport = FakeTransport::respond(200, {
                // `respond` wants 'static; these literals are.
                body
            });
            let provider = OpenAiCompatProvider::new(
                config(ProviderKind::Ollama),
                None,
                transport,
                noop_log(),
            )
            .unwrap();
            let error = provider.complete(&chat(), &CancelToken::new()).unwrap_err();
            assert_eq!(error, AiError::InvalidResponse, "{body}");
        }
    }

    #[test]
    fn invalid_requests_are_rejected_before_any_call() {
        let cases: Vec<(ChatRequest, &str)> = vec![
            (
                ChatRequest {
                    messages: vec![],
                    temperature: None,
                    max_tokens: None,
                },
                "at least one message",
            ),
            (
                ChatRequest {
                    temperature: Some(3.0),
                    ..chat()
                },
                "temperature",
            ),
            (
                ChatRequest {
                    max_tokens: Some(0),
                    ..chat()
                },
                "max_tokens",
            ),
        ];
        for (request, rule_part) in cases {
            let transport = FakeTransport::respond(200, COMPLETION);
            let provider = OpenAiCompatProvider::new(
                config(ProviderKind::Ollama),
                None,
                transport,
                noop_log(),
            )
            .unwrap();
            match provider
                .complete(&request, &CancelToken::new())
                .unwrap_err()
            {
                AiError::InvalidRequest { rule } => assert!(rule.contains(rule_part), "{rule}"),
                other => panic!("expected InvalidRequest, got {other:?}"),
            }
            assert!(provider.transport.seen.borrow().is_empty());
        }
    }

    #[test]
    fn connectivity_reports_whether_the_model_is_listed() {
        for (body, expected) in [
            (
                r#"{"data":[{"id":"test-model"},{"id":"other"}]}"#,
                Some(true),
            ),
            (r#"{"data":[{"id":"other"}]}"#, Some(false)),
            (r#"{"object":"list"}"#, None),
        ] {
            let transport = FakeTransport::respond(200, body);
            let provider = OpenAiCompatProvider::new(
                config(ProviderKind::Ollama),
                None,
                transport,
                noop_log(),
            )
            .unwrap();
            let connectivity = provider.check_connectivity(&CancelToken::new()).unwrap();
            assert_eq!(connectivity.model_available, expected, "{body}");
            let seen = provider.transport.seen.borrow();
            assert_eq!(seen[0].method, HttpMethod::Get);
            assert!(seen[0].url.ends_with("/models"));
        }
    }

    #[test]
    fn connectivity_treats_a_missing_models_endpoint_as_reachable() {
        for status in [404, 501] {
            let transport = FakeTransport::respond(status, "");
            let provider = OpenAiCompatProvider::new(
                config(ProviderKind::Ollama),
                None,
                transport,
                noop_log(),
            )
            .unwrap();
            let connectivity = provider.check_connectivity(&CancelToken::new()).unwrap();
            assert_eq!(connectivity.model_available, None);
        }
        let transport = FakeTransport::respond(401, "{}");
        let provider =
            OpenAiCompatProvider::new(config(ProviderKind::Ollama), None, transport, noop_log())
                .unwrap();
        assert_eq!(
            provider
                .check_connectivity(&CancelToken::new())
                .unwrap_err(),
            AiError::AuthRejected
        );
    }

    #[test]
    fn every_send_is_preceded_by_exactly_one_egress_log_entry() {
        let log = Arc::new(RecordingLog::default());
        let transport = FakeTransport::respond(200, COMPLETION);
        let provider =
            OpenAiCompatProvider::new(config(ProviderKind::Ollama), None, transport, log.clone())
                .unwrap();
        provider.complete(&chat(), &CancelToken::new()).unwrap();
        let sent_bytes = provider.transport.seen.borrow()[0]
            .body
            .as_ref()
            .unwrap()
            .len() as u64;
        let entries = log.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].provider_id, "p1");
        assert_eq!(entries[0].request_class, AiRequestClass::Completion);
        // The logged byte count is the exact body size that crossed the seam.
        assert_eq!(entries[0].request_bytes, sent_bytes);

        let log = Arc::new(RecordingLog::default());
        let transport = FakeTransport::respond(200, r#"{"data":[]}"#);
        let provider =
            OpenAiCompatProvider::new(config(ProviderKind::Ollama), None, transport, log.clone())
                .unwrap();
        provider.check_connectivity(&CancelToken::new()).unwrap();
        let entries = log.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].request_class, AiRequestClass::Connectivity);
        assert_eq!(entries[0].request_bytes, 0);
    }

    #[test]
    fn a_failing_egress_log_blocks_the_send_fail_closed() {
        // Breaking the log must not bypass it: no log entry, no egress.
        let transport = FakeTransport::respond(200, COMPLETION);
        let provider = OpenAiCompatProvider::new(
            config(ProviderKind::Ollama),
            None,
            transport,
            Arc::new(FailingLog),
        )
        .unwrap();
        assert_eq!(
            provider.complete(&chat(), &CancelToken::new()).unwrap_err(),
            AiError::EgressLogFailed
        );
        assert!(provider.transport.seen.borrow().is_empty());

        let transport = FakeTransport::respond(200, r#"{"data":[]}"#);
        let provider = OpenAiCompatProvider::new(
            config(ProviderKind::Ollama),
            None,
            transport,
            Arc::new(FailingLog),
        )
        .unwrap();
        assert_eq!(
            provider
                .check_connectivity(&CancelToken::new())
                .unwrap_err(),
            AiError::EgressLogFailed
        );
        assert!(provider.transport.seen.borrow().is_empty());
    }

    #[test]
    fn secret_bearing_content_is_refused_before_log_and_wire() {
        let log = Arc::new(RecordingLog::default());
        let transport = FakeTransport::respond(200, COMPLETION);
        let provider =
            OpenAiCompatProvider::new(config(ProviderKind::Ollama), None, transport, log.clone())
                .unwrap();
        let mut request = chat();
        request.messages.push(ChatMessage {
            role: Role::User,
            // Obviously-fake sample in the detector's AWS shape: sample
            // values must be visibly fake.
            content: "please summarize AKIAFAKEFAKEFAKEFAKE".into(),
        });
        match provider
            .complete(&request, &CancelToken::new())
            .unwrap_err()
        {
            AiError::SuspectedSecretBlocked { kinds } => assert!(!kinds.is_empty()),
            other => panic!("expected SuspectedSecretBlocked, got {other:?}"),
        }
        // Nothing crossed the seam and nothing was logged: the refusal is
        // a pure business error that leaves no trace of the content.
        assert!(provider.transport.seen.borrow().is_empty());
        assert!(log.entries.lock().unwrap().is_empty());
    }

    #[test]
    fn debug_renderings_carry_no_prompt_content_or_key() {
        let key = ApiKey::new("sk-FAKE-debug-canary").unwrap();
        let transport = FakeTransport::respond(200, COMPLETION);
        let provider = OpenAiCompatProvider::new(
            config(ProviderKind::OpenAiCompatible),
            Some(key),
            transport,
            noop_log(),
        )
        .unwrap();
        let request = chat();
        let outcome = ChatOutcome {
            content: "secret completion text".into(),
        };
        let rendered = format!("{provider:?} {request:?} {outcome:?}");
        assert!(!rendered.contains("organize this snippet"));
        assert!(!rendered.contains("secret completion text"));
        assert!(!rendered.contains("sk-FAKE-debug-canary"));
        assert!(rendered.contains("content_len"));
        assert!(rendered.contains("has_api_key"));
    }
}

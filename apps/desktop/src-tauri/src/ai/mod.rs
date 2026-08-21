// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! AI host wiring: assembles a provider call from the configured
//! `ai_provider` row, the secure-store API key, the HTTP transport and the
//! database egress-log sink. Business logic lives in
//! `typvia_host_service::ai`; the egress gate and the un-disableable log
//! discipline live in `crates/ai` — this module only plugs the pieces
//! together and maps errors onto the IPC codes.

mod commands;

pub use commands::*;

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use typvia_ai::credentials::load_api_key;
use typvia_ai::{
    AiError, AiProvider, CancelToken, ChatMessage, ChatRequest, CredentialError, EgressEntry,
    EgressLog, EgressLogError, HttpAiTransport, OpenAiCompatProvider, ProviderConfig, Role,
};
use typvia_core::repo::{AiEgressLogRepo, AiProviderRepo};
use typvia_core::vault::SecureStore;
use typvia_host_service::ai::OrganizePrompt;
use typvia_host_service::error::IpcError;

/// Production egress sink: entries land in `ai_egress_log` through the
/// app's single connection, stamped with the wall clock at write time.
/// Locks the connection only for the insert — callers must not hold the
/// connection lock across the provider call, or the mandatory pre-send
/// log write would deadlock (which is also why AI calls never run under
/// the lock in the first place).
struct DbEgressLog {
    conn: Arc<Mutex<Connection>>,
}

impl EgressLog for DbEgressLog {
    fn record(&self, entry: &EgressEntry) -> Result<(), EgressLogError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_millis()).ok())
            .ok_or(EgressLogError)?;
        let bytes = i64::try_from(entry.request_bytes).map_err(|_| EgressLogError)?;
        let conn = self.conn.lock().map_err(|_| EgressLogError)?;
        AiEgressLogRepo::new(&conn)
            .append(now, &entry.provider_id, entry.request_class, bytes)
            .map_err(|_| EgressLogError)
    }
}

/// One provider round trip as the commands describe it: the prompt pair
/// plus the per-call knobs. `model` overrides the provider row's model
/// (an action-level override); `None` keeps the row's default.
struct ProviderCall<'a> {
    system: &'a str,
    user: &'a str,
    temperature: Option<f32>,
    model: Option<&'a str>,
}

impl<'a> From<&'a OrganizePrompt> for ProviderCall<'a> {
    fn from(prompt: &'a OrganizePrompt) -> Self {
        ProviderCall {
            system: &prompt.system,
            user: &prompt.user,
            // Low temperature: organizing wants stable classification, not
            // creativity.
            temperature: Some(0.2),
            model: None,
        }
    }
}

fn chat_request(call: &ProviderCall) -> ChatRequest {
    ChatRequest {
        messages: vec![
            ChatMessage {
                role: Role::System,
                content: call.system.to_string(),
            },
            ChatMessage {
                role: Role::User,
                content: call.user.to_string(),
            },
        ],
        temperature: call.temperature,
        max_tokens: None,
    }
}

/// Builds a ready-to-call provider from the configured row, the stored
/// key and the mandatory egress sink. The row and key load under a short
/// lock; the returned provider performs I/O with the connection free.
fn assemble_provider(
    conn: Arc<Mutex<Connection>>,
    store: &dyn SecureStore,
    provider_id: &str,
    model_override: Option<&str>,
) -> Result<OpenAiCompatProvider<HttpAiTransport>, IpcError> {
    let row = {
        let guard = conn.lock().map_err(|_| IpcError::system())?;
        AiProviderRepo::new(&guard)
            .get(provider_id)?
            .ok_or_else(|| IpcError::conflict("this AI provider is not configured"))?
    };
    let model = model_override.unwrap_or(&row.model);
    let mut config =
        ProviderConfig::new(&row.id, row.kind, Some(&row.base_url), model).map_err(map_ai)?;
    config.timeout_ms = u64::try_from(row.timeout_ms).map_err(|_| IpcError::system())?;
    config.validate().map_err(map_ai)?;
    let key = load_api_key(store, &row.id).map_err(map_credentials)?;
    OpenAiCompatProvider::new(
        config,
        key,
        HttpAiTransport::new(),
        Arc::new(DbEgressLog { conn }),
    )
    .map_err(map_ai)
}

/// Sends one prompt pair through the configured provider and returns the
/// raw answer text.
fn complete_via_provider(
    conn: Arc<Mutex<Connection>>,
    store: &dyn SecureStore,
    provider_id: &str,
    call: ProviderCall<'_>,
) -> Result<String, IpcError> {
    let provider = assemble_provider(conn, store, provider_id, call.model)?;
    let request = chat_request(&call);
    let outcome = provider
        .complete(&request, &CancelToken::new())
        .map_err(map_ai)?;
    Ok(outcome.content)
}

/// Maps the crates/ai taxonomy onto the stable IPC codes. Messages are the
/// static AiError texts (payload-free by that crate's red-line tests).
fn map_ai(error: AiError) -> IpcError {
    match error {
        AiError::InvalidConfig { .. } | AiError::InvalidRequest { .. } => {
            IpcError::validation(error.to_string())
        }
        AiError::MissingApiKey
        | AiError::Cancelled
        | AiError::SuspectedSecretBlocked { .. }
        | AiError::SensitiveSnippetBlocked
        | AiError::AuthRejected
        | AiError::ModelNotFound
        | AiError::ProviderRejected { .. } => IpcError::conflict(error.to_string()),
        // Offline-shaped states: the UI shows "not reachable", not a failure.
        AiError::Timeout | AiError::Unreachable | AiError::RateLimited => {
            IpcError::unavailable(error.to_string())
        }
        AiError::ProviderFailure { .. }
        | AiError::InvalidResponse
        | AiError::Transport
        | AiError::EgressLogFailed => IpcError::system(),
    }
}

fn map_credentials(error: CredentialError) -> IpcError {
    match error {
        CredentialError::Rejected(rule) => IpcError::conflict(rule),
        CredentialError::Store(_) => IpcError::system(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use typvia_ai::AiRequestClass;
    use typvia_core::db::{migrate_to_latest, open_in_memory};
    use typvia_host_service::error::IpcErrorCode;

    use super::*;

    #[test]
    fn the_sink_writes_a_stamped_metadata_row() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        let sink = DbEgressLog {
            conn: Arc::new(Mutex::new(conn)),
        };
        sink.record(&EgressEntry {
            provider_id: "p1".to_string(),
            request_class: AiRequestClass::Completion,
            request_bytes: 321,
        })
        .unwrap();
        let guard = sink.conn.lock().unwrap();
        let rows = AiEgressLogRepo::new(&guard).list(10, 0).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].provider_id, "p1");
        assert_eq!(rows[0].request_bytes, 321);
        assert!(rows[0].occurred_at > 0);
    }

    #[test]
    fn an_unconfigured_provider_is_a_conflict_before_any_key_or_network_use() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        struct NoStore;
        impl SecureStore for NoStore {
            fn store(&self, _: &str, _: &[u8]) -> Result<(), typvia_crypto::SecureStoreError> {
                panic!("must not be reached");
            }
            fn retrieve(
                &self,
                _: &str,
            ) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, typvia_crypto::SecureStoreError>
            {
                panic!("must not be reached");
            }
            fn remove(&self, _: &str) -> Result<(), typvia_crypto::SecureStoreError> {
                panic!("must not be reached");
            }
        }
        let err = complete_via_provider(
            Arc::new(Mutex::new(conn)),
            &NoStore,
            "ghost",
            ProviderCall::from(&OrganizePrompt {
                system: "s".to_string(),
                user: "u".to_string(),
            }),
        )
        .unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
        assert!(err.message.contains("not configured"));
    }

    #[test]
    fn a_provider_call_carries_its_temperature_and_prompt_pair() {
        let request = chat_request(&ProviderCall {
            system: "instruction",
            user: "input",
            temperature: Some(0.7),
            model: Some("override-model"),
        });
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.messages[0].role, Role::System);
        assert_eq!(request.messages[0].content, "instruction");
        assert_eq!(request.messages[1].role, Role::User);
        assert_eq!(request.messages[1].content, "input");
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, None);
    }

    #[test]
    fn an_organize_call_keeps_the_low_classification_temperature() {
        let prompt = OrganizePrompt {
            system: "s".to_string(),
            user: "u".to_string(),
        };
        let call = ProviderCall::from(&prompt);
        assert_eq!(call.temperature, Some(0.2));
        assert!(call.model.is_none());
    }

    #[test]
    fn ai_errors_map_onto_the_stable_ipc_codes() {
        for (error, code) in [
            (
                AiError::InvalidConfig {
                    field: "base_url",
                    rule: "r",
                },
                IpcErrorCode::Validation,
            ),
            (AiError::MissingApiKey, IpcErrorCode::Conflict),
            (
                AiError::SuspectedSecretBlocked { kinds: vec![] },
                IpcErrorCode::Conflict,
            ),
            (AiError::SensitiveSnippetBlocked, IpcErrorCode::Conflict),
            (AiError::AuthRejected, IpcErrorCode::Conflict),
            (AiError::ModelNotFound, IpcErrorCode::Conflict),
            (AiError::Timeout, IpcErrorCode::Unavailable),
            (AiError::Unreachable, IpcErrorCode::Unavailable),
            (AiError::RateLimited, IpcErrorCode::Unavailable),
            (
                AiError::ProviderFailure { status: 500 },
                IpcErrorCode::System,
            ),
            (AiError::EgressLogFailed, IpcErrorCode::System),
        ] {
            assert_eq!(map_ai(error).code, code);
        }
    }
}

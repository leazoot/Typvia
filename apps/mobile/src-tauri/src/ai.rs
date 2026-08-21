// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Mobile AI host wiring: the same assembly the desktop host performs — the
//! configured `ai_provider` row, the secure-store API key, the HTTP transport
//! and the database egress-log sink — for the mobile main app only. The
//! keyboard extension and the IME never load this module or the crates it
//! wires: they stay zero-AI, zero-network. Business logic lives in
//! `typvia_host_service::ai`; the egress gate and the un-disableable log
//! discipline live in `crates/ai`.
//!
//! This is deliberate host glue duplicated from
//! `apps/desktop/src-tauri/src/ai` rather than shared: host-service may not
//! depend on crates/ai, so each host plugs the transport and sink together
//! itself, exactly like each host owns its own secure-store backend.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use typvia_ai::credentials::{load_api_key, remove_api_key, store_api_key};
use typvia_ai::{
    AiError, AiProvider, CancelToken, ChatMessage, ChatRequest, CredentialError, EgressEntry,
    EgressLog, EgressLogError, HttpAiTransport, OpenAiCompatProvider, ProviderConfig, ProviderKind,
    Role,
};
use typvia_core::repo::{AiEgressLogRepo, AiProviderRepo};
use typvia_core::vault::SecureStore;
use typvia_host_service::ai::{
    self as host_ai, ActionRunInput, AiActionDto, AiEgressPageDto, AiProviderDto,
    AiProviderSaveInput, OrganizeDraftInput, OrganizePrompt, OrganizeSuggestionDto,
    organize_prompt, organize_validate,
};
use typvia_host_service::error::IpcError;

use crate::commands::{AppState, now_ms};

/// Production egress sink: entries land in `ai_egress_log` through the
/// app's single connection. Locks the connection only for the insert —
/// callers must not hold the connection lock across the provider call.
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

/// One provider round trip: the prompt pair plus the per-call knobs.
/// `model` overrides the provider row's model; `None` keeps the default.
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

#[tauri::command]
pub fn ai_provider_list(state: State<'_, AppState>) -> Result<Vec<AiProviderDto>, IpcError> {
    let conn = state.lock()?;
    host_ai::ai_provider_list(&conn, |id| {
        matches!(load_api_key(state.secure_store(), id), Ok(Some(_)))
    })
}

#[tauri::command]
pub fn ai_provider_save(
    state: State<'_, AppState>,
    input: AiProviderSaveInput,
) -> Result<AiProviderDto, IpcError> {
    // Resolve through the egress-policy owner first: kind defaults fill a
    // blank base URL, the https/loopback rule rejects bad ones, and the
    // timeout bounds apply — before anything is stored.
    let kind = input
        .kind
        .parse::<ProviderKind>()
        .map_err(|_| IpcError::validation("unknown provider kind"))?;
    let base = {
        let trimmed = input.base_url.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    };
    let mut config = ProviderConfig::new("pending", kind, base, &input.model).map_err(map_ai)?;
    if input.timeout_ms > 0 {
        config.timeout_ms = u64::try_from(input.timeout_ms).map_err(|_| IpcError::system())?;
        config.validate().map_err(map_ai)?;
    }
    let resolved = AiProviderSaveInput {
        base_url: config.base_url.clone(),
        timeout_ms: i64::try_from(config.timeout_ms).map_err(|_| IpcError::system())?,
        ..input
    };
    let conn = state.lock()?;
    host_ai::ai_provider_save(&conn, resolved, now_ms()?)
}

#[tauri::command]
pub fn ai_provider_delete(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    // Key first, row second: if the row delete then fails the user simply
    // re-enters the key — never the reverse (no orphaned credentials).
    remove_api_key(state.secure_store(), &id).map_err(map_credentials)?;
    let conn = state.lock()?;
    host_ai::ai_provider_delete(&conn, &id)
}

#[tauri::command]
pub fn ai_api_key_set(
    state: State<'_, AppState>,
    provider_id: String,
    key: String,
) -> Result<(), IpcError> {
    {
        let conn = state.lock()?;
        AiProviderRepo::new(&conn)
            .get(&provider_id)?
            .ok_or_else(|| IpcError::conflict("this AI provider is not configured"))?;
    }
    store_api_key(state.secure_store(), &provider_id, &key).map_err(map_credentials)
}

#[tauri::command]
pub fn ai_api_key_clear(state: State<'_, AppState>, provider_id: String) -> Result<(), IpcError> {
    remove_api_key(state.secure_store(), &provider_id).map_err(map_credentials)
}

#[tauri::command]
pub fn ai_organize(
    state: State<'_, AppState>,
    provider_id: String,
    input: OrganizeDraftInput,
) -> Result<OrganizeSuggestionDto, IpcError> {
    let prompt = {
        let conn = state.lock()?;
        organize_prompt(&conn, &input)?
    };
    let answer = complete_via_provider(
        state.conn_handle(),
        state.secure_store(),
        &provider_id,
        (&prompt).into(),
    )?;
    let conn = state.lock()?;
    organize_validate(&conn, &answer)
}

#[tauri::command]
pub fn ai_action_list(state: State<'_, AppState>) -> Result<Vec<AiActionDto>, IpcError> {
    let mut conn = state.lock()?;
    host_ai::ai_action_list(&mut conn, now_ms()?)
}

/// A finished action run: the validated output plus which detector kinds
/// were masked out of the input. The result stays pending until the user
/// confirms; applying it is the UI's job.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiActionRunResultDto {
    pub output: String,
    pub masked_kinds: Vec<String>,
}

#[tauri::command]
pub fn ai_action_run(
    state: State<'_, AppState>,
    action_id: String,
    input: ActionRunInput,
) -> Result<AiActionRunResultDto, IpcError> {
    // Same three-phase shape as organize: scope checks and prompt build
    // under a short lock, the provider round trip lock-free (through the
    // egress gate), then answer validation.
    let prompt = {
        let conn = state.lock()?;
        host_ai::ai_action_run_prompt(&conn, &action_id, &input)?
    };
    let answer = complete_via_provider(
        state.conn_handle(),
        state.secure_store(),
        &prompt.provider_id,
        ProviderCall {
            system: &prompt.system,
            user: &prompt.user,
            temperature: prompt.temperature,
            model: prompt.model.as_deref(),
        },
    )?;
    let result = host_ai::ai_action_run_validate(&answer)?;
    Ok(AiActionRunResultDto {
        output: result.output,
        masked_kinds: prompt.masked_kinds,
    })
}

#[tauri::command]
pub fn ai_egress_log_list(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> Result<AiEgressPageDto, IpcError> {
    let conn = state.lock()?;
    host_ai::ai_egress_log_list(&conn, limit, offset)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use typvia_ai::AiRequestClass;
    use typvia_core::db::{migrate_to_latest, open_in_memory};
    use typvia_host_service::error::IpcErrorCode;

    use super::*;

    #[test]
    fn the_mobile_sink_writes_a_stamped_metadata_row() {
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
            fn store(&self, _: &str, _: &[u8]) -> Result<(), typvia_core::vault::SecureStoreError> {
                panic!("must not be reached");
            }
            fn retrieve(
                &self,
                _: &str,
            ) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, typvia_core::vault::SecureStoreError>
            {
                panic!("must not be reached");
            }
            fn remove(&self, _: &str) -> Result<(), typvia_core::vault::SecureStoreError> {
                panic!("must not be reached");
            }
        }
        let err = complete_via_provider(
            Arc::new(Mutex::new(conn)),
            &NoStore,
            "ghost",
            ProviderCall {
                system: "s",
                user: "u",
                temperature: None,
                model: None,
            },
        )
        .unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
        assert!(err.message.contains("not configured"));
    }
}

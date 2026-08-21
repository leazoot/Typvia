// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! AI IPC commands. The organize command runs in three phases so the AI
//! round trip never holds the database lock: prompt build
//! (short lock) → provider call (lock free; the egress sink re-locks
//! briefly) → answer validation (short lock). Nothing here writes snippet
//! data — confirmation goes through the existing save commands.
//!
//! Every command that reaches the network is async and does its blocking
//! work on the runtime's blocking pool: a synchronous Tauri command runs on
//! the main thread, so a slow provider (timeouts up to minutes) would
//! beachball the whole app.
//!
//! Provider configuration resolves kind defaults, the URL egress policy
//! and timeout bounds through crates/ai (the policy owner) before the row
//! reaches host-service storage. API keys pass straight into the secure
//! store and never appear in any response DTO.

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use typvia_ai::credentials::{load_api_key, remove_api_key, store_api_key};
use typvia_ai::{AiProvider, CancelToken, ProviderConfig, ProviderKind};
use typvia_host_service::ai::{
    self as host_ai, ActionRunInput, AiActionDto, AiActionSaveInput, AiEgressPageDto,
    AiProviderDto, AiProviderSaveInput, ExtractDraftInput, OrganizeDraftInput,
    OrganizeSuggestionDto, VariableProposalDto, organize_prompt, organize_validate,
};
use typvia_host_service::error::IpcError;

use crate::commands::{AppState, now_ms};

#[tauri::command]
pub async fn ai_organize(
    app: AppHandle,
    provider_id: String,
    input: OrganizeDraftInput,
) -> Result<OrganizeSuggestionDto, IpcError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let prompt = {
            let conn = state.lock()?;
            organize_prompt(&conn, &input)?
        };
        let answer = super::complete_via_provider(
            state.conn_handle(),
            state.secure_store(),
            &provider_id,
            (&prompt).into(),
        )?;
        let conn = state.lock()?;
        organize_validate(&conn, &answer)
    })
    .await
    .map_err(|_| IpcError::system())?
}

#[tauri::command]
pub async fn ai_extract_variables(
    app: AppHandle,
    provider_id: String,
    input: ExtractDraftInput,
) -> Result<Vec<VariableProposalDto>, IpcError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        // Both extraction phases are database-free; only the provider
        // assembly and the egress sink take short locks inside.
        let prompt = host_ai::extract_prompt(&input)?;
        let answer = super::complete_via_provider(
            state.conn_handle(),
            state.secure_store(),
            &provider_id,
            (&prompt).into(),
        )?;
        host_ai::extract_validate(&input.body, &answer)
    })
    .await
    .map_err(|_| IpcError::system())?
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
    let mut config =
        ProviderConfig::new("pending", kind, base, &input.model).map_err(super::map_ai)?;
    if input.timeout_ms > 0 {
        config.timeout_ms = u64::try_from(input.timeout_ms).map_err(|_| IpcError::system())?;
        config.validate().map_err(super::map_ai)?;
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
    remove_api_key(state.secure_store(), &id).map_err(super::map_credentials)?;
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
        typvia_core::repo::AiProviderRepo::new(&conn)
            .get(&provider_id)?
            .ok_or_else(|| IpcError::conflict("this AI provider is not configured"))?;
    }
    store_api_key(state.secure_store(), &provider_id, &key).map_err(super::map_credentials)
}

#[tauri::command]
pub fn ai_api_key_clear(state: State<'_, AppState>, provider_id: String) -> Result<(), IpcError> {
    remove_api_key(state.secure_store(), &provider_id).map_err(super::map_credentials)
}

#[tauri::command]
pub fn ai_action_list(state: State<'_, AppState>) -> Result<Vec<AiActionDto>, IpcError> {
    let mut conn = state.lock()?;
    host_ai::ai_action_list(&mut conn, now_ms()?)
}

#[tauri::command]
pub fn ai_action_save(
    state: State<'_, AppState>,
    input: AiActionSaveInput,
) -> Result<AiActionDto, IpcError> {
    let conn = state.lock()?;
    host_ai::ai_action_save(&conn, input, now_ms()?)
}

#[tauri::command]
pub fn ai_action_delete(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    let conn = state.lock()?;
    host_ai::ai_action_delete(&conn, &id)
}

/// A finished action run: the validated output plus which detector kinds
/// were masked out of the input (`mask_secrets` scope — the UI shows them
/// as "secrets stripped before sending"). The result stays pending until
/// the user confirms; applying it is the UI's job.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiActionRunResultDto {
    pub output: String,
    pub masked_kinds: Vec<String>,
}

#[tauri::command]
pub async fn ai_action_run(
    app: AppHandle,
    action_id: String,
    input: ActionRunInput,
) -> Result<AiActionRunResultDto, IpcError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        // Same three-phase shape as organize: scope checks and prompt build
        // under a short lock, the provider round trip lock-free (through the
        // egress gate), then answer validation.
        let prompt = {
            let conn = state.lock()?;
            host_ai::ai_action_run_prompt(&conn, &action_id, &input)?
        };
        let answer = super::complete_via_provider(
            state.conn_handle(),
            state.secure_store(),
            &prompt.provider_id,
            super::ProviderCall {
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
    })
    .await
    .map_err(|_| IpcError::system())?
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

/// Connectivity probe result for the settings row. `model_available` is
/// `null` when the endpoint is reachable but does not enumerate models.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConnectivityDto {
    pub model_available: Option<bool>,
}

#[tauri::command]
pub async fn ai_check_connectivity(
    app: AppHandle,
    provider_id: String,
) -> Result<AiConnectivityDto, IpcError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let provider = super::assemble_provider(
            state.conn_handle(),
            state.secure_store(),
            &provider_id,
            None,
        )?;
        let connectivity = provider
            .check_connectivity(&CancelToken::new())
            .map_err(super::map_ai)?;
        Ok(AiConnectivityDto {
            model_available: connectivity.model_available,
        })
    })
    .await
    .map_err(|_| IpcError::system())?
}

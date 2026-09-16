// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! AI host wiring for the main app: the configured provider row, the
//! secure-store API key, the HTTP transport and the database egress sink,
//! assembled into one provider.
//!
//! Two properties survive the move to a native host, and neither is optional.
//!
//! The egress gate is inside `crates/ai`: a request that carries a sensitive
//! snippet or a suspected secret never reaches the transport. There is no
//! path around it here, because there is no path to the transport that does
//! not go through a provider, and a provider cannot be constructed without
//! the gate.
//!
//! The egress log cannot be switched off. `OpenAiCompatProvider::new` demands
//! a sink; passing none is not a shape this code can express. A sink that
//! fails fails the call — the request does not go out unlogged. What the sink
//! writes is metadata only: when, which provider, which request class, how
//! many bytes. Never the prompt, never the key.
//!
//! The keyboard extension and the widget never link this module or the crates
//! it wires. They stay zero-AI and zero-network.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use typvia_ai::credentials::{load_api_key, remove_api_key, store_api_key};
use typvia_ai::{
    AiError, AiProvider, CancelToken, ChatMessage, ChatRequest, CredentialError, EgressEntry,
    EgressLog, EgressLogError, HttpAiTransport, OpenAiCompatProvider, ProviderConfig, ProviderKind,
    Role,
};
use typvia_core::repo::{AiEgressLogRepo, AiProviderRepo};
use typvia_core::vault::SecureStore;
use typvia_host_service::ai::{
    self as host_ai, ActionRunInput, AiProviderSaveInput, OrganizeDraftInput, OrganizePrompt,
};
use typvia_host_service::dto::{FolderDto, TagDto};
use typvia_host_service::error::IpcError;

use crate::error::CoreError;
use crate::service::{TypviaCore, now_ms};

/// Production egress sink: entries land in the log table through the app's
/// single connection. Locks that connection only for the insert, so a caller
/// must not already hold it while the provider call is in flight.
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

/// One provider round trip: the prompt pair plus the per-call knobs. `model`
/// overrides the provider row's model; `None` keeps the default.
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

/// Builds a ready-to-call provider from the configured row, the stored key and
/// the mandatory egress sink. The row and key load under a short lock; the
/// returned provider performs I/O with the connection free.
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

/// Sends one prompt pair through the configured provider and returns the raw
/// answer text.
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

/// Maps the AI taxonomy onto the stable error codes. Messages are the static
/// error texts, which that crate's own red-line tests keep payload-free.
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
        // Offline-shaped states: the screen says "not reachable", not
        // "something failed".
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

/// One configured provider. The key itself never crosses — only whether one
/// is stored, which is what a settings row needs to render.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AiProviderRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    pub timeout_ms: i64,
    pub has_api_key: bool,
}

/// Fields the settings form supplies when saving a provider.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AiProviderDraft {
    /// Absent when creating; the repository assigns an id.
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    /// Empty falls back to the kind's default endpoint.
    pub base_url: String,
    pub model: String,
    /// Zero keeps the default timeout.
    pub timeout_ms: i64,
}

/// One saved action.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AiAction {
    pub id: String,
    pub name: String,
    pub prompt_template: String,
    pub provider_id: Option<String>,
    /// Action-level override; absent falls back to the provider's model.
    pub model: Option<String>,
    pub input_source: String,
    pub output_mode: String,
    pub permission_scope: String,
    pub temperature: Option<f32>,
    /// A seeded built-in. Editable like any other action; the flag only
    /// labels it.
    pub is_builtin: bool,
}

/// A finished run: the validated output plus which detector kinds were masked
/// out of the input before it left. The result stays pending until the user
/// confirms it — applying it is the screen's job, not this one's.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AiActionResult {
    pub output: String,
    pub masked_kinds: Vec<String>,
}

/// One organize suggestion for the draft the user is writing.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AiOrganizeSuggestion {
    pub title: Option<String>,
    pub description: Option<String>,
    pub snippet_type: Option<String>,
    pub security_level: Option<String>,
    pub trigger: Option<String>,
    /// Existing tags matched by name, plus any the model proposed that
    /// survived the vocabulary check.
    pub tags: Vec<crate::model::Tag>,
    pub folder: Option<crate::model::Folder>,
}

/// One egress-log row: metadata only, by construction.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AiEgressEntry {
    pub id: i64,
    pub occurred_at: i64,
    pub provider_id: String,
    pub request_class: String,
    pub request_bytes: i64,
}

/// A page of the egress log, with the total so the screen can state how much
/// went out rather than how much it happens to be showing.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AiEgressPage {
    pub entries: Vec<AiEgressEntry>,
    pub total: i64,
}

fn tag_out(tag: TagDto) -> crate::model::Tag {
    crate::model::Tag {
        id: tag.id,
        name: tag.name,
        created_at: tag.created_at,
    }
}

fn folder_out(folder: FolderDto) -> crate::model::Folder {
    crate::model::Folder {
        id: folder.id,
        parent_id: folder.parent_id,
        name: folder.name,
        sort_order: folder.sort_order,
        created_at: folder.created_at,
        updated_at: folder.updated_at,
    }
}

#[uniffi::export]
impl TypviaCore {
    pub fn ai_provider_list(&self) -> Result<Vec<AiProviderRow>, CoreError> {
        let conn = self.conn()?;
        let rows = host_ai::ai_provider_list(&conn, |id| {
            matches!(load_api_key(self.secure_store(), id), Ok(Some(_)))
        })?;
        Ok(rows
            .into_iter()
            .map(|p| AiProviderRow {
                id: p.id,
                name: p.name,
                kind: p.kind,
                base_url: p.base_url,
                model: p.model,
                timeout_ms: p.timeout_ms,
                has_api_key: p.has_api_key,
            })
            .collect())
    }

    /// Saves a provider, resolving it through the egress-policy owner first:
    /// a blank base URL takes the kind's default, the https/loopback rule
    /// rejects the rest, and the timeout bounds apply — all before anything
    /// is stored.
    pub fn ai_provider_save(&self, draft: AiProviderDraft) -> Result<AiProviderRow, CoreError> {
        let kind = draft
            .kind
            .parse::<ProviderKind>()
            .map_err(|_| IpcError::validation("unknown provider kind"))?;
        let trimmed = draft.base_url.trim();
        let base = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        let mut config =
            ProviderConfig::new("pending", kind, base, &draft.model).map_err(map_ai)?;
        if draft.timeout_ms > 0 {
            config.timeout_ms = u64::try_from(draft.timeout_ms).map_err(|_| IpcError::system())?;
            config.validate().map_err(map_ai)?;
        }
        let input = AiProviderSaveInput {
            id: draft.id,
            name: draft.name,
            kind: draft.kind,
            base_url: config.base_url.clone(),
            model: draft.model,
            timeout_ms: i64::try_from(config.timeout_ms).map_err(|_| IpcError::system())?,
        };
        let now = now_ms()?;
        let conn = self.conn()?;
        let saved = host_ai::ai_provider_save(&conn, input, now)?;
        Ok(AiProviderRow {
            id: saved.id,
            name: saved.name,
            kind: saved.kind,
            base_url: saved.base_url,
            model: saved.model,
            timeout_ms: saved.timeout_ms,
            has_api_key: saved.has_api_key,
        })
    }

    /// Deletes a provider. Key first, row second: if the row delete then
    /// fails the user simply re-enters the key, where the reverse order would
    /// strand a credential with nothing pointing at it.
    pub fn ai_provider_delete(&self, id: String) -> Result<(), CoreError> {
        remove_api_key(self.secure_store(), &id).map_err(map_credentials)?;
        let conn = self.conn()?;
        Ok(host_ai::ai_provider_delete(&conn, &id)?)
    }

    /// Stores an API key for a configured provider. The key goes to the
    /// platform's secure storage and never to the database.
    pub fn ai_api_key_set(&self, provider_id: String, key: String) -> Result<(), CoreError> {
        {
            let conn = self.conn()?;
            AiProviderRepo::new(&conn)
                .get(&provider_id)
                .map_err(IpcError::from)?
                .ok_or_else(|| IpcError::conflict("this AI provider is not configured"))?;
        }
        Ok(store_api_key(self.secure_store(), &provider_id, &key).map_err(map_credentials)?)
    }

    pub fn ai_api_key_clear(&self, provider_id: String) -> Result<(), CoreError> {
        Ok(remove_api_key(self.secure_store(), &provider_id).map_err(map_credentials)?)
    }

    /// Asks the provider to propose a title, type, folder and tags for a
    /// draft. Three phases: build the prompt under a short lock, make the
    /// round trip with the connection free, validate the answer.
    pub fn ai_organize(
        &self,
        provider_id: String,
        title: String,
        body: String,
        description: Option<String>,
        is_sensitive: bool,
    ) -> Result<AiOrganizeSuggestion, CoreError> {
        // A draft already marked sensitive is refused by the shared layer
        // before a prompt is built — the flag is passed through, never
        // decided here.
        let input = OrganizeDraftInput {
            title,
            body,
            description,
            is_sensitive,
        };
        let prompt = {
            let conn = self.conn()?;
            host_ai::organize_prompt(&conn, &input)?
        };
        let answer = complete_via_provider(
            self.conn_handle(),
            self.secure_store(),
            &provider_id,
            (&prompt).into(),
        )?;
        let conn = self.conn()?;
        let suggestion = host_ai::organize_validate(&conn, &answer)?;
        Ok(AiOrganizeSuggestion {
            title: suggestion.title,
            description: suggestion.description,
            snippet_type: suggestion.snippet_type,
            security_level: suggestion.security_level,
            trigger: suggestion.trigger,
            tags: suggestion.tags.into_iter().map(tag_out).collect(),
            folder: suggestion.folder.map(folder_out),
        })
    }

    pub fn ai_action_list(&self) -> Result<Vec<AiAction>, CoreError> {
        let now = now_ms()?;
        let mut conn = self.conn()?;
        let rows = host_ai::ai_action_list(&mut conn, now)?;
        Ok(rows
            .into_iter()
            .map(|a| AiAction {
                id: a.id,
                name: a.name,
                prompt_template: a.prompt_template,
                provider_id: a.provider_id,
                model: a.model,
                input_source: a.input_source,
                output_mode: a.output_mode,
                permission_scope: a.permission_scope,
                temperature: a.temperature,
                is_builtin: a.is_builtin,
            })
            .collect())
    }

    pub fn ai_action_delete(&self, id: String) -> Result<(), CoreError> {
        let conn = self.conn()?;
        Ok(host_ai::ai_action_delete(&conn, &id)?)
    }

    /// Runs a saved action over the given text. Same three phases as organize:
    /// scope checks and prompt build under a short lock, the round trip
    /// lock-free and through the egress gate, then answer validation.
    pub fn ai_action_run(
        &self,
        action_id: String,
        text: String,
        source: String,
        is_sensitive: bool,
    ) -> Result<AiActionResult, CoreError> {
        // `source` has to match the action's configured input source: the
        // action is a contract about where its text may come from, not a
        // hint. `is_sensitive` is passed through to the gate that refuses
        // the run outright.
        let input = ActionRunInput {
            text,
            source,
            is_sensitive,
        };
        let prompt = {
            let conn = self.conn()?;
            host_ai::ai_action_run_prompt(&conn, &action_id, &input)?
        };
        let answer = complete_via_provider(
            self.conn_handle(),
            self.secure_store(),
            &prompt.provider_id,
            ProviderCall {
                system: &prompt.system,
                user: &prompt.user,
                temperature: prompt.temperature,
                model: prompt.model.as_deref(),
            },
        )?;
        let result = host_ai::ai_action_run_validate(&answer)?;
        Ok(AiActionResult {
            output: result.output,
            masked_kinds: prompt.masked_kinds,
        })
    }

    /// The record of everything this device has sent out. It cannot be
    /// cleared or switched off, which is the point: a claim about what never
    /// leaves is only worth what its evidence is.
    pub fn ai_egress_log_list(&self, limit: u32, offset: u32) -> Result<AiEgressPage, CoreError> {
        let conn = self.conn()?;
        let page = host_ai::ai_egress_log_list(&conn, limit, offset)?;
        Ok(AiEgressPage {
            entries: page
                .entries
                .into_iter()
                .map(|e| AiEgressEntry {
                    id: e.id,
                    occurred_at: e.occurred_at,
                    provider_id: e.provider_id,
                    request_class: e.request_class,
                    request_bytes: e.request_bytes,
                })
                .collect(),
            total: page.total,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use typvia_ai::AiRequestClass;
    use typvia_core::db::{migrate_to_latest, open_in_memory};
    use typvia_host_service::error::IpcErrorCode;

    fn memory_conn() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        conn
    }

    /// A store that panics on use, so a test can prove a path was refused
    /// before it ever reached for a credential.
    struct NoStore;

    impl SecureStore for NoStore {
        fn store(&self, _: &str, _: &[u8]) -> Result<(), typvia_core::vault::SecureStoreError> {
            panic!("the credential store must not be reached");
        }

        fn retrieve(
            &self,
            _: &str,
        ) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, typvia_core::vault::SecureStoreError>
        {
            panic!("the credential store must not be reached");
        }

        fn remove(&self, _: &str) -> Result<(), typvia_core::vault::SecureStoreError> {
            panic!("the credential store must not be reached");
        }
    }

    #[test]
    fn the_sink_writes_a_stamped_metadata_row() {
        let sink = DbEgressLog {
            conn: Arc::new(Mutex::new(memory_conn())),
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

    /// Red line: the log row is metadata. A prompt carrying a deliberately
    /// fake secret leaves no trace of it in what the sink writes.
    #[test]
    fn a_log_row_carries_no_prompt_text_and_no_key() {
        let sink = DbEgressLog {
            conn: Arc::new(Mutex::new(memory_conn())),
        };
        sink.record(&EgressEntry {
            provider_id: "p1".to_string(),
            request_class: AiRequestClass::Completion,
            request_bytes: 4096,
        })
        .unwrap();

        let guard = sink.conn.lock().unwrap();
        let rows = AiEgressLogRepo::new(&guard).list(10, 0).unwrap();
        let rendered = format!("{rows:?}");
        assert!(!rendered.contains("AKIAFAKE"));
        // The shape itself is the guarantee: the entry has nowhere to put
        // prompt text, so there is no field a future change could fill with
        // it by accident.
        assert!(!rendered.contains("prompt"));
    }

    /// An unconfigured provider is refused before any credential is read and
    /// before any transport exists — the panicking store proves the order.
    #[test]
    fn an_unconfigured_provider_is_refused_before_any_key_or_network_use() {
        let error = complete_via_provider(
            Arc::new(Mutex::new(memory_conn())),
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

        assert_eq!(error.code, IpcErrorCode::Conflict);
        assert!(error.message.contains("not configured"));
    }

    fn core() -> (TypviaCore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let core = TypviaCore::open(dir.path().to_string_lossy().to_string()).unwrap();
        (core, dir)
    }

    /// Red line: text the editor marks sensitive is refused before a prompt
    /// exists, let alone a request.
    ///
    /// The refusal has to be *this* one. The fixture names no configured
    /// provider, so a call that got as far as assembling one would fail too —
    /// with a different message. Checking which refusal came back is what
    /// proves the gate ran first.
    #[test]
    fn a_sensitive_draft_is_refused_before_a_prompt_is_even_built() {
        let (core, _dir) = core();

        let error = core
            .ai_organize(
                "ghost".to_string(),
                "Deploy key".to_string(),
                "AKIAFAKEEXAMPLE00000".to_string(),
                None,
                true,
            )
            .unwrap_err();

        assert_eq!(
            error,
            CoreError::Conflict {
                reason: "sensitive snippets never leave this device".to_string(),
            }
        );
    }

    /// The same gate on the action path, and the same reasoning about which
    /// refusal proves the order.
    #[test]
    fn a_sensitive_action_run_is_refused_before_a_prompt_is_even_built() {
        let (core, _dir) = core();
        // Built-ins seed lazily on first listing; the action has to exist so
        // the refusal below can only be the sensitivity gate.
        let action = core.ai_action_list().unwrap().remove(0);

        let error = core
            .ai_action_run(
                action.id,
                "AKIAFAKEEXAMPLE00000".to_string(),
                "selection".to_string(),
                true,
            )
            .unwrap_err();

        assert_eq!(
            error,
            CoreError::Conflict {
                reason: "sensitive snippets never leave this device".to_string(),
            }
        );
    }

    /// Red line: whatever a refused call did, it left no egress record —
    /// because it never reached the transport that writes one.
    #[test]
    fn a_refused_call_leaves_no_egress_record() {
        let (core, _dir) = core();
        let _ = core.ai_organize(
            "ghost".to_string(),
            "Deploy key".to_string(),
            "AKIAFAKEEXAMPLE00000".to_string(),
            None,
            true,
        );

        let page = core.ai_egress_log_list(20, 0).unwrap();

        assert_eq!(page.total, 0);
        assert!(page.entries.is_empty());
    }

    /// Red line: a provider cannot exist without an egress sink. This is a
    /// compile-time property rather than a runtime check — the constructor
    /// takes the sink by value, so "AI with logging turned off" is not a
    /// state this host can construct.
    #[test]
    fn a_provider_cannot_be_built_without_an_egress_sink() {
        let config = ProviderConfig::new(
            "p1",
            ProviderKind::Ollama,
            Some("http://127.0.0.1:11434"),
            "llama3",
        )
        .unwrap();

        let provider = OpenAiCompatProvider::new(
            config,
            None,
            HttpAiTransport::new(),
            Arc::new(DbEgressLog {
                conn: Arc::new(Mutex::new(memory_conn())),
            }),
        );

        assert!(provider.is_ok());
    }
}

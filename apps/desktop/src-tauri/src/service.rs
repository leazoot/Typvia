//! Desktop-specific service functions: the injector- and Espanso-coupled use
//! cases. The shared pure-Connection IPC orchestration (snippets, search,
//! vault, templates, backup, app rules) lives in crates/host-service; this
//! file holds only what needs the desktop platform seams.

use std::path::Path;

use rusqlite::Connection;
use typvia_core::import as generic_import;
use typvia_core::model::SnippetContent;
use typvia_core::repo::SnippetRepo;
use typvia_core::vault::VaultSession;
use typvia_espanso_adapter::{
    CoexistenceChoice, EngineGate, EngineState, EspansoCli, EspansoVersion, ImportParseError,
    ImportedMatch, compile_snippets, parse_matches, write_config,
};
use typvia_host_service::dto::{EspansoImportDto, EspansoSkippedDto, EspansoStatusDto};
use typvia_host_service::error::IpcError;
use typvia_host_service::service::{commit_imported, entry_snippet, template_render};
use zeroize::Zeroizing;

use crate::injector::{InjectionMethod, Injector, InjectorError};

/// Maps an injection failure onto the shared IPC error shape. InjectorError
/// Display strings are static and payload-free, so they are safe to forward.
/// PermissionDenied is a recoverable business error (offer copy); the rest
/// are non-correctable system failures.
fn injector_error(error: InjectorError) -> IpcError {
    match error {
        InjectorError::PermissionDenied => IpcError::permission_denied(error.to_string()),
        InjectorError::Clipboard | InjectorError::Synthesis | InjectorError::Unsupported => {
            IpcError::system()
        }
    }
}

/// Renders a template with the supplied values and injects it into the
/// frontmost app, recording one usage on success (mirrors `snippet_inject`).
pub fn template_inject(
    conn: &Connection,
    injector: &mut dyn Injector,
    id: &str,
    values: &std::collections::HashMap<String, String>,
    method: InjectionMethod,
    now: i64,
) -> Result<(), IpcError> {
    let text = template_render(conn, id, values)?;
    injector.inject(&text, method).map_err(injector_error)?;
    SnippetRepo::new(conn).record_usage(id, now)?;
    Ok(())
}

/// Loads a normal snippet's plaintext body for delivery. v1 injection covers
/// normal snippets only; sensitive (ciphertext) injection arrives with the
/// vault after unlock/verification.
fn deliverable_body(conn: &Connection, id: &str) -> Result<String, IpcError> {
    let snippet = SnippetRepo::new(conn)
        .get(id)?
        .ok_or_else(IpcError::not_found)?;
    match snippet.content {
        SnippetContent::Plaintext(text) => Ok(text),
        SnippetContent::Ciphertext(_) => Err(IpcError::validation(
            "sensitive snippets cannot be injected yet",
        )),
    }
}

/// Injects a snippet into the frontmost application, then records one usage.
/// Usage is recorded only after a successful delivery, so a failed or
/// permission-denied injection leaves usage_count untouched.
pub fn snippet_inject(
    conn: &Connection,
    injector: &mut dyn Injector,
    id: &str,
    method: InjectionMethod,
    now: i64,
) -> Result<(), IpcError> {
    let body = deliverable_body(conn, id)?;
    injector.inject(&body, method).map_err(injector_error)?;
    SnippetRepo::new(conn).record_usage(id, now)?;
    Ok(())
}

/// Copies a snippet to the clipboard (also the no-permission fallback for
/// injection), then records one usage on success.
pub fn snippet_copy(
    conn: &Connection,
    injector: &mut dyn Injector,
    id: &str,
    now: i64,
) -> Result<(), IpcError> {
    let body = deliverable_body(conn, id)?;
    injector.copy(&body).map_err(injector_error)?;
    SnippetRepo::new(conn).record_usage(id, now)?;
    Ok(())
}

// ===== Espanso integration =====

/// Reports the managed engine's status: the coexistence gate wins,
/// then the supervisor's own state; the binary being missing is its own
/// degraded state. `enabled` is the caller's default-on switch reading (only
/// an explicit user off flips it). A low-frequency call (settings open / home
/// load) — the command layer runs the gated autostart first, so an enabled
/// engine that can start is already `running` by the time this maps states.
pub fn espanso_status<C: EspansoCli>(
    conn: &Connection,
    cli: C,
    typvia_config: &Path,
    enabled: bool,
    gate: EngineGate,
    engine: EngineState,
    choice: Option<CoexistenceChoice>,
) -> Result<EspansoStatusDto, IpcError> {
    let version = match cli.run(&["--version"]) {
        Ok(output) if output.code == Some(0) => {
            Some(EspansoVersion::parse(&output.stdout).raw().to_string())
        }
        _ => None,
    };
    let state = if version.is_none() {
        "unavailable"
    } else {
        match gate {
            EngineGate::Disabled => "off",
            EngineGate::ConflictUnresolved => "conflict",
            EngineGate::TakeoverPending => "takeover_pending",
            EngineGate::StandingAside => "standing_aside",
            EngineGate::Start => match engine {
                EngineState::Running { .. } => "running",
                EngineState::Retrying { .. } => "retrying",
                // Idle/Stopped with an open gate means the start attempt did
                // not stick — degraded, same surface as an exhausted restart.
                EngineState::Failed | EngineState::Idle | EngineState::Stopped => "failed",
            },
        }
    };
    let snippets = SnippetRepo::new(conn).list_triggered_active()?;
    let trigger_count = compile_snippets(&snippets)
        .map(|compiled| compiled.match_count)
        .unwrap_or(0);
    Ok(EspansoStatusDto {
        state: state.to_string(),
        version,
        config_path: Some(typvia_config.display().to_string()),
        enabled,
        trigger_count,
        coexistence_choice: choice.map(|choice| {
            match choice {
                CoexistenceChoice::Takeover => "takeover",
                CoexistenceChoice::StandAside => "stand_aside",
            }
            .to_string()
        }),
    })
}

/// Compiles the current triggered snippet set and atomically writes the espanso
/// config to `config_path`. A compile failure preserves the previous config
/// (nothing is written). Returns the number of triggers written.
pub fn espanso_regenerate(conn: &Connection, config_path: &Path) -> Result<usize, IpcError> {
    let snippets = SnippetRepo::new(conn).list_triggered_active()?;
    let compiled = compile_snippets(&snippets)
        .map_err(|_| IpcError::validation("a triggered snippet has an invalid trigger"))?;
    write_config(config_path, &compiled.yaml).map_err(|_| IpcError::system())?;
    Ok(compiled.match_count)
}

/// Removes Typvia's espanso config file (turns the integration off). A missing
/// file is treated as success (already off).
pub fn espanso_remove(config_path: &Path) -> Result<(), IpcError> {
    match std::fs::remove_file(config_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(IpcError::system()),
    }
}

/// Imports an espanso match file's plain trigger/replace pairs as normal
/// snippets. Unsupported matches and trigger conflicts are reported, not
/// imported. All inserts happen in one transaction, so a failure leaves no
/// half-imported state (external input is parsed as a business error).
pub fn espanso_import(
    conn: &Connection,
    yaml: &str,
    now: i64,
) -> Result<EspansoImportDto, IpcError> {
    let parsed = parse_matches(yaml).map_err(|error| match error {
        ImportParseError::InvalidYaml => IpcError::validation("could not parse the Espanso file"),
        ImportParseError::NotAMatchFile => IpcError::validation("not an Espanso match file"),
    })?;

    let snippets = parsed
        .matches
        .iter()
        .map(|entry| entry_snippet(&unified_match(entry), now))
        .collect();
    let (imported_ids, conflicts) = commit_imported(conn, snippets, now)?;

    Ok(EspansoImportDto {
        imported: imported_ids.len(),
        conflicts,
        skipped: parsed
            .skipped
            .into_iter()
            .map(|skip| EspansoSkippedDto {
                trigger: skip.trigger,
                reason: skip.reason,
            })
            .collect(),
    })
}

/// Lifts an espanso match into the unified import model.
fn unified_match(entry: &ImportedMatch) -> generic_import::ImportedEntry {
    generic_import::ImportedEntry {
        title: None,
        content: entry.replace.clone(),
        trigger: Some(entry.trigger.clone()),
        word: entry.word,
        description: None,
    }
}

/// Decrypts a sensitive snippet's body for host-side delivery (inject/copy).
/// Unlike `vault_reveal`, the plaintext never crosses IPC — it is handed
/// straight to the injector and wiped. Requires an unlocked session and a
/// snippet that is actually sensitive.
fn secret_delivery_body(
    conn: &Connection,
    session: &mut VaultSession,
    id: &str,
    now: i64,
) -> Result<Zeroizing<String>, IpcError> {
    let snippet = SnippetRepo::new(conn)
        .get(id)?
        .ok_or_else(IpcError::not_found)?;
    let envelope = match snippet.content {
        SnippetContent::Ciphertext(bytes) => bytes,
        SnippetContent::Plaintext(_) => {
            return Err(IpcError::conflict("snippet is not sensitive"));
        }
    };
    let plaintext = session.decrypt_content(conn, id, &envelope)?;
    // Delivering is user activity: defer the idle auto-lock.
    session.note_activity(now);
    let text = String::from_utf8(plaintext.to_vec()).map_err(|_| IpcError::system())?;
    Ok(Zeroizing::new(text))
}

/// Injects a sensitive snippet into the frontmost app after unlock. The
/// decrypted body is wiped after delivery; on the paste path the injector
/// restores the prior clipboard, so the secret does not linger there.
pub fn snippet_inject_secret(
    conn: &Connection,
    session: &mut VaultSession,
    injector: &mut dyn Injector,
    id: &str,
    method: InjectionMethod,
    now: i64,
) -> Result<(), IpcError> {
    let body = secret_delivery_body(conn, session, id, now)?;
    injector.inject(&body, method).map_err(injector_error)?;
    SnippetRepo::new(conn).record_usage(id, now)?;
    Ok(())
}

/// Copies a sensitive snippet to the clipboard for manual paste, guarded so the
/// timed auto-clear wipes it only if still present. The decrypted
/// body is wiped from this process at once; the clipboard copy is the caller's
/// to schedule the clear for.
pub fn snippet_copy_secret(
    conn: &Connection,
    session: &mut VaultSession,
    injector: &mut dyn Injector,
    id: &str,
    now: i64,
) -> Result<(), IpcError> {
    let body = secret_delivery_body(conn, session, id, now)?;
    injector.copy_guarded(&body).map_err(injector_error)?;
    SnippetRepo::new(conn).record_usage(id, now)?;
    Ok(())
}

/// Clears a guarded sensitive clipboard copy if it is still present (the timed
/// auto-clear). Returns whether it actually cleared.
pub fn clipboard_clear_secret(injector: &mut dyn Injector) -> Result<bool, IpcError> {
    injector.clear_guarded().map_err(injector_error)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use typvia_core::model::{SecurityLevel, Snippet, SnippetType, TriggerMode};
    use typvia_core::repo::new_id;
    use typvia_host_service::dto::{SnippetCreateInput, TemplateFieldDto};
    use typvia_host_service::error::IpcErrorCode;
    use typvia_host_service::service::{
        snippet_create, snippet_get, template_save_fields, vault_create_secret,
    };

    fn test_conn() -> Connection {
        let mut conn = typvia_core::db::open_in_memory().unwrap();
        typvia_core::db::migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn espanso_snippet(
        title: &str,
        trigger: &str,
        level: SecurityLevel,
        content: SnippetContent,
    ) -> Snippet {
        Snippet {
            id: new_id(),
            workspace_id: "default".to_string(),
            title: title.to_string(),
            content,
            snippet_type: match level {
                SecurityLevel::Sensitive => SnippetType::Sensitive,
                SecurityLevel::Normal => SnippetType::Text,
            },
            description: None,
            folder_id: None,
            trigger: Some(trigger.to_string()),
            trigger_mode: Some(TriggerMode::Immediate),
            language: None,
            security_level: level,
            is_favorite: false,
            is_pinned: false,
            is_enabled: true,
            platform_scope: vec![],
            created_at: 1,
            updated_at: 1,
            last_used_at: None,
            usage_count: 0,
            version: 1,
            deleted_at: None,
            conflict_of: None,
        }
    }

    struct VersionOnlyCli {
        installed: bool,
    }

    impl EspansoCli for VersionOnlyCli {
        fn run(&self, args: &[&str]) -> std::io::Result<typvia_espanso_adapter::CliOutput> {
            assert_eq!(args, ["--version"], "status only reads the version");
            if self.installed {
                Ok(typvia_espanso_adapter::CliOutput {
                    code: Some(0),
                    stdout: "2.4.0\n".to_string(),
                    stderr: String::new(),
                })
            } else {
                Err(std::io::Error::from(std::io::ErrorKind::NotFound))
            }
        }
    }

    #[test]
    fn espanso_status_maps_gate_engine_and_binary_states() {
        let conn = test_conn();
        let missing_config = std::path::Path::new("/nowhere/typvia.yml");
        let cases: [(EngineGate, EngineState, &str); 6] = [
            (EngineGate::Disabled, EngineState::Idle, "off"),
            (
                EngineGate::ConflictUnresolved,
                EngineState::Idle,
                "conflict",
            ),
            (
                EngineGate::TakeoverPending,
                EngineState::Idle,
                "takeover_pending",
            ),
            (
                EngineGate::StandingAside,
                EngineState::Idle,
                "standing_aside",
            ),
            (
                EngineGate::Start,
                EngineState::Running { pid: 42 },
                "running",
            ),
            // An open gate with no live engine is the degraded surface.
            (EngineGate::Start, EngineState::Idle, "failed"),
        ];
        for (gate, engine, expected) in cases {
            let dto = espanso_status(
                &conn,
                VersionOnlyCli { installed: true },
                missing_config,
                true,
                gate,
                engine,
                None,
            )
            .unwrap();
            assert_eq!(dto.state, expected);
            assert_eq!(dto.version.as_deref(), Some("2.4.0"));
            assert!(dto.enabled, "default-on switch reading passes through");
        }
    }

    #[test]
    fn espanso_status_reports_a_missing_binary_as_unavailable() {
        let conn = test_conn();
        let dto = espanso_status(
            &conn,
            VersionOnlyCli { installed: false },
            std::path::Path::new("/nowhere/typvia.yml"),
            false,
            EngineGate::Start,
            EngineState::Running { pid: 42 },
            Some(CoexistenceChoice::StandAside),
        )
        .unwrap();
        assert_eq!(dto.state, "unavailable");
        assert!(!dto.enabled);
        assert_eq!(dto.version, None);
        assert_eq!(dto.coexistence_choice.as_deref(), Some("stand_aside"));
    }

    #[test]
    fn espanso_regenerate_writes_normal_triggers_and_omits_sensitive() {
        let conn = test_conn();
        let repo = SnippetRepo::new(&conn);
        repo.insert(&espanso_snippet(
            "Signature",
            ":hello",
            SecurityLevel::Normal,
            SnippetContent::Plaintext("Hi there".to_string()),
        ))
        .unwrap();
        repo.insert(&espanso_snippet(
            "Secret",
            ":secret",
            SecurityLevel::Sensitive,
            SnippetContent::Ciphertext(vec![9, 9, 9]),
        ))
        .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("match").join("typvia.yml");
        let count = espanso_regenerate(&conn, &target).unwrap();

        assert_eq!(count, 1, "only the normal snippet is written");
        let written = std::fs::read_to_string(&target).unwrap();
        assert!(written.contains(":hello"));
        assert!(!written.contains(":secret"), "sensitive trigger leaked");
    }

    #[test]
    fn espanso_import_creates_snippets_reports_conflicts_and_skips() {
        let conn = test_conn();
        SnippetRepo::new(&conn)
            .insert(&espanso_snippet(
                "Existing",
                ":taken",
                SecurityLevel::Normal,
                SnippetContent::Plaintext("old".to_string()),
            ))
            .unwrap();

        let yaml = "matches:\n  \
             - trigger: \":sig\"\n    replace: \"Best regards\"\n  \
             - trigger: \":taken\"\n    replace: \"conflict\"\n  \
             - regex: \":n(x)\"\n    replace: \"num\"\n";
        let summary = espanso_import(&conn, yaml, 1_000).unwrap();

        assert_eq!(summary.imported, 1);
        assert_eq!(summary.conflicts, vec![":taken".to_string()]);
        assert_eq!(summary.skipped.len(), 1);

        // The imported snippet is real and search-indexed.
        let triggers: Vec<Option<String>> = SnippetRepo::new(&conn)
            .list_triggered_active()
            .unwrap()
            .into_iter()
            .map(|s| s.trigger)
            .collect();
        assert!(triggers.contains(&Some(":sig".to_string())));
    }

    #[test]
    fn espanso_import_rejects_a_non_match_file() {
        let conn = test_conn();
        let error = espanso_import(&conn, "global_vars: []", 1).unwrap_err();
        assert_eq!(error.code, IpcErrorCode::Validation);
    }

    fn template_snippet(conn: &Connection, id: &str, body: &str) {
        SnippetRepo::new(conn)
            .insert(&Snippet {
                id: id.to_string(),
                workspace_id: "default".to_string(),
                title: "Bug report".to_string(),
                content: SnippetContent::Plaintext(body.to_string()),
                snippet_type: SnippetType::Template,
                description: None,
                folder_id: None,
                trigger: None,
                trigger_mode: None,
                language: None,
                security_level: SecurityLevel::Normal,
                is_favorite: false,
                is_pinned: false,
                is_enabled: true,
                platform_scope: vec![],
                created_at: 1,
                updated_at: 1,
                last_used_at: None,
                usage_count: 0,
                version: 1,
                deleted_at: None,
                conflict_of: None,
            })
            .unwrap();
    }

    fn field_dto(name: &str, field_type: &str) -> TemplateFieldDto {
        TemplateFieldDto {
            id: String::new(),
            name: name.to_string(),
            label: name.to_string(),
            field_type: field_type.to_string(),
            default_value: None,
            options: vec![],
            validation: None,
            is_required: false,
            sort_order: 0,
            platform_overrides: None,
        }
    }

    fn fill(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn template_render_fills_values_and_inject_records_one_usage() {
        let conn = test_conn();
        template_snippet(&conn, "s1", "### {{module}} · {{severity}}");
        let mut module = field_dto("module", "single_line_text");
        module.is_required = true;
        template_save_fields(
            &conn,
            "s1",
            vec![module, field_dto("severity", "single_line_text")],
            9_000,
        )
        .unwrap();

        let values = fill(&[("module", "auth"), ("severity", "high")]);
        assert_eq!(
            template_render(&conn, "s1", &values).unwrap(),
            "### auth · high"
        );

        let mut injector = FakeInjector::granted();
        template_inject(
            &conn,
            &mut injector,
            "s1",
            &values,
            InjectionMethod::Paste,
            500,
        )
        .unwrap();
        assert_eq!(injector.injected, vec!["### auth · high".to_string()]);
        let after = snippet_get(&conn, "s1").unwrap();
        assert_eq!(after.usage_count, 1);
        assert_eq!(after.last_used_at, Some(500));
    }

    const VAULT_NOW: i64 = 1_700_000_000_000;

    fn unlocked_session(conn: &Connection) -> VaultSession {
        let mut session = VaultSession::new();
        session
            .initialize(conn, b"correct horse battery staple", VAULT_NOW)
            .unwrap();
        session
    }

    fn secret_input(title: &str, body: &str) -> SnippetCreateInput {
        SnippetCreateInput {
            title: title.to_string(),
            body: body.to_string(),
            snippet_type: "sensitive".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
        }
    }

    #[test]
    fn injecting_a_secret_decrypts_it_and_records_one_usage() {
        let conn = test_conn();
        let mut session = unlocked_session(&conn);
        let secret = "sk_live_EXAMPLEONLY_do_not_use";
        let dto = vault_create_secret(
            &conn,
            &session,
            secret_input("Deploy key", secret),
            VAULT_NOW,
        )
        .unwrap();
        let mut injector = FakeInjector::granted();

        snippet_inject_secret(
            &conn,
            &mut session,
            &mut injector,
            &dto.id,
            InjectionMethod::Paste,
            VAULT_NOW,
        )
        .unwrap();

        assert_eq!(injector.injected, vec![secret.to_string()]);
        let after = snippet_get(&conn, &dto.id).unwrap();
        assert_eq!(after.usage_count, 1);
    }

    #[test]
    fn injecting_a_secret_requires_an_unlocked_session() {
        let conn = test_conn();
        let mut session = unlocked_session(&conn);
        let dto =
            vault_create_secret(&conn, &session, secret_input("Key", "shh"), VAULT_NOW).unwrap();
        session.lock();
        let mut injector = FakeInjector::granted();

        let err = snippet_inject_secret(
            &conn,
            &mut session,
            &mut injector,
            &dto.id,
            InjectionMethod::Paste,
            VAULT_NOW,
        )
        .unwrap_err();

        assert_eq!(err.code, IpcErrorCode::PermissionDenied);
        // A refused delivery injects nothing and does not count as a use.
        assert!(injector.injected.is_empty());
        assert_eq!(snippet_get(&conn, &dto.id).unwrap().usage_count, 0);
    }

    #[test]
    fn copying_a_secret_guards_the_clipboard_then_clears_only_once() {
        let conn = test_conn();
        let mut session = unlocked_session(&conn);
        let secret = "AKIA_FAKE_EXAMPLE_SECRET";
        let dto =
            vault_create_secret(&conn, &session, secret_input("AWS", secret), VAULT_NOW).unwrap();
        let mut injector = FakeInjector::granted();

        snippet_copy_secret(&conn, &mut session, &mut injector, &dto.id, VAULT_NOW).unwrap();
        assert_eq!(injector.copied, vec![secret.to_string()]);
        assert_eq!(snippet_get(&conn, &dto.id).unwrap().usage_count, 1);

        // The timed clear wipes the guarded copy once; a second clear is a no-op.
        assert!(clipboard_clear_secret(&mut injector).unwrap());
        assert!(!clipboard_clear_secret(&mut injector).unwrap());
    }

    #[test]
    fn delivering_a_normal_snippet_as_a_secret_is_refused() {
        let conn = test_conn();
        let mut session = unlocked_session(&conn);
        let created = snippet_create(&conn, create_input("Plain", None), VAULT_NOW).unwrap();
        let mut injector = FakeInjector::granted();

        let err = snippet_copy_secret(&conn, &mut session, &mut injector, &created.id, VAULT_NOW)
            .unwrap_err();

        assert_eq!(err.code, IpcErrorCode::Conflict);
        assert!(injector.copied.is_empty());
    }

    /// Records what it was asked to deliver, or fails with a set error, so
    /// tests can drive the inject/copy use cases without a GUI.
    struct FakeInjector {
        granted: bool,
        fail_with: Option<InjectorError>,
        injected: Vec<String>,
        copied: Vec<String>,
        /// Set by `copy_guarded`, taken by `clear_guarded` (mirrors the real
        /// injector's "clear only if still ours" guard, minus the live read).
        guarded: Option<String>,
    }

    impl FakeInjector {
        fn granted() -> Self {
            Self {
                granted: true,
                fail_with: None,
                injected: Vec::new(),
                copied: Vec::new(),
                guarded: None,
            }
        }

        fn failing(error: InjectorError) -> Self {
            Self {
                granted: true,
                fail_with: Some(error),
                injected: Vec::new(),
                copied: Vec::new(),
                guarded: None,
            }
        }
    }

    impl Injector for FakeInjector {
        fn accessibility_granted(&self) -> bool {
            self.granted
        }

        fn inject(&mut self, text: &str, _method: InjectionMethod) -> Result<(), InjectorError> {
            if let Some(error) = self.fail_with {
                return Err(error);
            }
            self.injected.push(text.to_string());
            Ok(())
        }

        fn copy(&mut self, text: &str) -> Result<(), InjectorError> {
            if let Some(error) = self.fail_with {
                return Err(error);
            }
            self.copied.push(text.to_string());
            Ok(())
        }

        fn copy_guarded(&mut self, text: &str) -> Result<(), InjectorError> {
            if let Some(error) = self.fail_with {
                return Err(error);
            }
            self.copied.push(text.to_string());
            self.guarded = Some(text.to_string());
            Ok(())
        }

        fn clear_guarded(&mut self) -> Result<bool, InjectorError> {
            if let Some(error) = self.fail_with {
                return Err(error);
            }
            Ok(self.guarded.take().is_some())
        }
    }

    #[test]
    fn inject_delivers_body_and_records_one_usage() {
        let conn = test_conn();
        let created = snippet_create(&conn, create_input("Deploy", None), 1).unwrap();
        let mut injector = FakeInjector::granted();

        snippet_inject(
            &conn,
            &mut injector,
            &created.id,
            InjectionMethod::Paste,
            100,
        )
        .unwrap();

        assert_eq!(injector.injected, vec!["Deploy body".to_string()]);
        let after = snippet_get(&conn, &created.id).unwrap();
        assert_eq!(after.usage_count, 1);
        assert_eq!(after.last_used_at, Some(100));
    }

    #[test]
    fn copy_delivers_body_and_records_one_usage() {
        let conn = test_conn();
        let created = snippet_create(&conn, create_input("Token", None), 1).unwrap();
        let mut injector = FakeInjector::granted();

        snippet_copy(&conn, &mut injector, &created.id, 200).unwrap();

        assert_eq!(injector.copied, vec!["Token body".to_string()]);
        let after = snippet_get(&conn, &created.id).unwrap();
        assert_eq!(after.usage_count, 1);
        assert_eq!(after.last_used_at, Some(200));
    }

    #[test]
    fn failed_injection_records_no_usage() {
        let conn = test_conn();
        let created = snippet_create(&conn, create_input("Deploy", None), 1).unwrap();
        let mut injector = FakeInjector::failing(InjectorError::PermissionDenied);

        let err = snippet_inject(
            &conn,
            &mut injector,
            &created.id,
            InjectionMethod::Paste,
            100,
        )
        .unwrap_err();

        // Permission denial is a recoverable business error (offer copy), and
        // a failed delivery must not bump usage.
        assert_eq!(err.code, IpcErrorCode::PermissionDenied);
        let after = snippet_get(&conn, &created.id).unwrap();
        assert_eq!(after.usage_count, 0);
        assert_eq!(after.last_used_at, None);
    }

    /// Real-hardware proof that the actual platform injector, driven through
    /// the service, records the usage that feeds the Home ledger. Posts a real
    /// ⌘V into the frontmost app, so it is ignored by default. Cross-app
    /// delivery + clipboard restore are proven separately by
    /// `tests/injector_live.rs`.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "posts a real ⌘V and needs Accessibility permission; run manually"]
    fn real_injector_inject_records_usage_on_hardware() {
        use crate::injector::platform_injector_or_null;

        let conn = test_conn();
        let created = snippet_create(&conn, create_input("Ledger", None), 1).unwrap();
        let mut injector = platform_injector_or_null();
        assert!(
            injector.accessibility_granted(),
            "grant Accessibility to the invoking terminal before running"
        );

        snippet_inject(
            &conn,
            injector.as_mut(),
            &created.id,
            InjectionMethod::Paste,
            500,
        )
        .unwrap();

        let after = snippet_get(&conn, &created.id).unwrap();
        assert_eq!(after.usage_count, 1);
        assert_eq!(after.last_used_at, Some(500));
    }

    fn create_input(title: &str, trigger: Option<&str>) -> SnippetCreateInput {
        SnippetCreateInput {
            title: title.to_string(),
            body: format!("{title} body"),
            snippet_type: "command".to_string(),
            description: None,
            folder_id: None,
            trigger: trigger.map(str::to_string),
            trigger_mode: trigger.map(|_| "delimiter".to_string()),
            language: None,
        }
    }
}

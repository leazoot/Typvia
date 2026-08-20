//! Built-in AI action bootstrap.
//!
//! Seeds the eight built-in actions exactly once per device,
//! guarded by the `ai_actions_seeded` app_meta marker: after seeding the
//! rows are ordinary user data, so a deleted built-in must stay deleted —
//! per-row upserts would resurrect it. Ids are fixed (`builtin.<slug>`) so
//! devices that each seed independently converge to the same entities
//! under sync instead of accumulating duplicates.
//!
//! Seeds ship unconfigured (no provider, no model) and default to the
//! strictest permission scope; execution surfaces "provider not
//! configured" until the user picks one.

use rusqlite::Connection;

use crate::model::{
    AiAction, AiActionInputSource, AiActionOutputMode, AiActionParams, AiActionPermissionScope,
    TimestampMs,
};
use crate::repo::{AiActionRepo, AppMetaRepo, RepoError};

/// app_meta key marking that the one-shot seed has run.
pub const SEED_MARKER_KEY: &str = "ai_actions_seeded";

struct SeedSpec {
    id: &'static str,
    name: &'static str,
    prompt_template: &'static str,
    input_source: AiActionInputSource,
    output_mode: AiActionOutputMode,
}

const SEEDS: [SeedSpec; 8] = [
    SeedSpec {
        id: "builtin.rewrite",
        name: "Rewrite",
        prompt_template: "Rewrite the text so it reads clearly and naturally. Keep the \
                          original language, meaning and tone; fix grammar and awkward \
                          phrasing. Return only the rewritten text.",
        input_source: AiActionInputSource::Selection,
        output_mode: AiActionOutputMode::Replace,
    },
    SeedSpec {
        id: "builtin.translate",
        name: "Translate",
        prompt_template: "Translate the text into English if it is in another language, \
                          otherwise into Chinese. Preserve formatting, code and \
                          placeholders. Return only the translation.",
        input_source: AiActionInputSource::Selection,
        output_mode: AiActionOutputMode::Replace,
    },
    SeedSpec {
        id: "builtin.summarize",
        name: "Summarize",
        prompt_template: "Summarize the text in a few short sentences, keeping its \
                          language. Cover the key points and drop the detail. Return \
                          only the summary.",
        input_source: AiActionInputSource::Selection,
        output_mode: AiActionOutputMode::Copy,
    },
    SeedSpec {
        id: "builtin.code-review",
        name: "Code review",
        prompt_template: "Review the code. Point out bugs, risky patterns and unclear \
                          parts, most important first, each with a short explanation \
                          and a concrete suggestion. Do not rewrite the whole code.",
        input_source: AiActionInputSource::Selection,
        output_mode: AiActionOutputMode::Copy,
    },
    SeedSpec {
        id: "builtin.commit-message",
        name: "Commit message",
        prompt_template: "Write a conventional commit message for this change: a \
                          `<type>: <short imperative description>` subject line under \
                          72 characters, plus a brief body only if the change needs \
                          one. Return only the commit message.",
        input_source: AiActionInputSource::Clipboard,
        output_mode: AiActionOutputMode::Copy,
    },
    SeedSpec {
        id: "builtin.convert-format",
        name: "Convert format",
        prompt_template: "Convert the text into clean Markdown, preserving its \
                          structure (headings, lists, tables, code). If it already is \
                          Markdown, tidy it instead. Return only the converted text.",
        input_source: AiActionInputSource::Selection,
        output_mode: AiActionOutputMode::Replace,
    },
    SeedSpec {
        id: "builtin.improve-prompt",
        name: "Improve prompt",
        prompt_template: "Improve this AI prompt: make the goal, context, constraints \
                          and expected output explicit while keeping the author's \
                          intent and language. Return only the improved prompt.",
        input_source: AiActionInputSource::Selection,
        output_mode: AiActionOutputMode::Replace,
    },
    SeedSpec {
        id: "builtin.extract-variables",
        name: "Extract variables",
        prompt_template: "Find the parts of the text that would change between uses \
                          (names, dates, amounts, links) and rewrite it as a template, \
                          replacing each with a {{snake_case}} placeholder. Return \
                          only the template text.",
        input_source: AiActionInputSource::Snippet,
        output_mode: AiActionOutputMode::NewSnippet,
    },
];

/// Seeds the built-in actions if this device has never seeded before.
/// Returns `true` when the seed ran, `false` when the marker was already
/// set. The eight inserts and the marker commit atomically — a failure
/// leaves neither rows nor marker behind.
pub fn ensure_builtin_actions(conn: &mut Connection, now: TimestampMs) -> Result<bool, RepoError> {
    if AppMetaRepo::new(conn).get(SEED_MARKER_KEY)?.is_some() {
        return Ok(false);
    }
    let tx = conn.transaction().map_err(RepoError::from)?;
    {
        let actions = AiActionRepo::new(&tx);
        for seed in &SEEDS {
            actions.insert(&AiAction {
                id: seed.id.to_string(),
                name: seed.name.to_string(),
                prompt_template: seed.prompt_template.to_string(),
                provider_id: None,
                model: None,
                input_source: seed.input_source,
                output_mode: seed.output_mode,
                permission_scope: AiActionPermissionScope::NormalOnly,
                params: AiActionParams::default(),
                created_at: now,
                updated_at: now,
            })?;
        }
        AppMetaRepo::new(&tx).set(SEED_MARKER_KEY, "1")?;
    }
    tx.commit().map_err(RepoError::from)?;
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::{migrate_to_latest, open_in_memory};

    fn setup() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        conn
    }

    #[test]
    fn seeds_the_eight_builtins_unconfigured_and_strict() {
        let mut conn = setup();
        assert!(ensure_builtin_actions(&mut conn, 42).unwrap());
        let actions = AiActionRepo::new(&conn).list(50, 0).unwrap();
        assert_eq!(actions.len(), 8);
        for action in &actions {
            assert!(action.id.starts_with("builtin."));
            assert_eq!(action.provider_id, None);
            assert_eq!(action.model, None);
            assert_eq!(action.permission_scope, AiActionPermissionScope::NormalOnly);
            assert_eq!(action.created_at, 42);
        }
    }

    #[test]
    fn runs_once_and_never_resurrects_a_deleted_builtin() {
        let mut conn = setup();
        assert!(ensure_builtin_actions(&mut conn, 1).unwrap());
        AiActionRepo::new(&conn)
            .delete("builtin.translate")
            .unwrap();

        // A later startup sees the marker and leaves user data alone.
        assert!(!ensure_builtin_actions(&mut conn, 2).unwrap());
        let repo = AiActionRepo::new(&conn);
        assert_eq!(repo.get("builtin.translate").unwrap(), None);
        assert_eq!(repo.list(50, 0).unwrap().len(), 7);
    }
}

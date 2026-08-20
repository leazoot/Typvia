//! Keyboard snapshot generation.
//!
//! Builds the versioned JSON read model the iOS keyboard and Android IME
//! consume. Content policy: only live, enabled snippets enter a snapshot;
//! normal snippets contribute their plaintext fields, sensitive snippets
//! contribute nothing beyond their id and the stored content-ciphertext
//! envelope. Output is deterministic for identical database state.

use rusqlite::Connection;

use crate::model::{
    FolderMetadata, KeyboardSnapshot, SNAPSHOT_VERSION, SecurityLevel, SnapshotSnippet, Snippet,
    SnippetContent, SnippetId, TimestampMs,
};
use crate::repo::{FolderRepo, RepoError, SnippetRepo};

/// Generates the snapshot document from current database state.
///
/// `recent_ids` lists used snippets by most recent use, `favorite_ids`
/// follows the Library "Starred" display order, and `folder_metadata`
/// carries every folder. The result is validated before it is returned, so
/// callers can hand it straight to serialization.
pub fn generate(
    conn: &Connection,
    device_id: &str,
    now: TimestampMs,
) -> Result<KeyboardSnapshot, RepoError> {
    let snippets = SnippetRepo::new(conn).list_snapshot_active()?;
    let folders = FolderRepo::new(conn).list_all_parents_first()?;

    let mut entries = Vec::with_capacity(snippets.len());
    for snippet in &snippets {
        entries.push(entry_of(snippet)?);
    }

    let snapshot = KeyboardSnapshot {
        snapshot_version: SNAPSHOT_VERSION,
        generated_at: now,
        device_id: device_id.to_string(),
        snippets: entries,
        recent_ids: recent_ids(&snippets),
        favorite_ids: favorite_ids(&snippets),
        folder_metadata: folders
            .into_iter()
            .map(|folder| FolderMetadata {
                id: folder.id,
                name: folder.name,
                sort_order: folder.sort_order,
            })
            .collect(),
    };
    snapshot.validate()?;
    Ok(snapshot)
}

/// Maps one stored snippet to its snapshot entry. The match is exhaustive
/// over (security level, content form), so a sensitive row can never reach
/// the plaintext branch; a mismatched row surfaces as a corrupted-row
/// conflict instead of leaking anything.
fn entry_of(snippet: &Snippet) -> Result<SnapshotSnippet, RepoError> {
    match (snippet.security_level, &snippet.content) {
        (SecurityLevel::Normal, SnippetContent::Plaintext(body)) => Ok(SnapshotSnippet::Normal {
            id: snippet.id.clone(),
            title: snippet.title.clone(),
            snippet_type: snippet.snippet_type.as_str().to_string(),
            trigger: snippet.trigger.clone(),
            trigger_mode: snippet.trigger_mode.map(|mode| mode.as_str().to_string()),
            folder_id: snippet.folder_id.clone(),
            is_favorite: snippet.is_favorite,
            body: body.clone(),
        }),
        (SecurityLevel::Sensitive, SnippetContent::Ciphertext(envelope)) => {
            Ok(SnapshotSnippet::Sensitive {
                id: snippet.id.clone(),
                encrypted_metadata: envelope.clone(),
            })
        }
        _ => Err(RepoError::Conflict(
            "snippet row has invalid content columns",
        )),
    }
}

/// Ids of snippets used at least once, most recently used first; ties break
/// on id so the output stays stable.
fn recent_ids(snippets: &[Snippet]) -> Vec<SnippetId> {
    let mut used: Vec<&Snippet> = snippets
        .iter()
        .filter(|snippet| snippet.last_used_at.is_some())
        .collect();
    used.sort_by(|a, b| {
        b.last_used_at
            .cmp(&a.last_used_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    used.into_iter().map(|snippet| snippet.id.clone()).collect()
}

/// Favorite snippet ids in the Library "Starred" display order (most
/// recently updated first); ties break on id so the output stays stable.
fn favorite_ids(snippets: &[Snippet]) -> Vec<SnippetId> {
    let mut favorites: Vec<&Snippet> = snippets
        .iter()
        .filter(|snippet| snippet.is_favorite)
        .collect();
    favorites.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    favorites
        .into_iter()
        .map(|snippet| snippet.id.clone())
        .collect()
}

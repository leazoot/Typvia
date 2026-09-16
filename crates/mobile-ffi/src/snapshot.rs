// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Read-only snapshot surface for the restricted extension processes.
//!
//! The iOS keyboard and widget and the Android IME cannot open the database:
//! they get the App Group snapshot document instead, and this module is the
//! typed way to read it. Nothing here writes, and nothing here decrypts —
//! a sensitive snippet is present as an id and a lock flag, never as content.

use std::collections::HashSet;

use typvia_core::model::{KeyboardSnapshot, SnapshotSnippet};

/// Errors an extension can hit while reading a snapshot document.
///
/// Variants deliberately carry no document content: a snapshot holds
/// encrypted sensitive metadata, and extension error paths can end up in
/// system logs. `Invalid` carries the
/// failing field name only.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum SnapshotError {
    /// The document is not parseable snapshot JSON.
    Malformed,
    /// The document parsed but failed validation.
    Invalid { field: String },
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => write!(f, "not a readable snapshot document"),
            Self::Invalid { field } => write!(f, "snapshot rejected: {field}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

/// What a keyboard needs to know about a snapshot before trusting it.
///
/// The overview deliberately surfaces counts and folder titles only; snippet
/// entries (plaintext for normal snippets, opaque ciphertext for sensitive
/// ones) are consumed by the platform snippet views, not exposed here.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SnapshotOverview {
    pub snapshot_version: u32,
    pub generated_at: i64,
    pub device_id: String,
    /// How many entries the snapshot carries.
    ///
    /// This is the denominator an extension is allowed to print. It is not
    /// "everything in the library": a snapshot holds the live, enabled rows
    /// only, so the honest reading is "what this keyboard can reach". A
    /// keyboard that printed a ratio out of the app's own total would be
    /// printing a number it has no way to check.
    pub entry_total: u32,
    pub recent_total: u32,
    pub favorite_total: u32,
    /// Folder names in display order (`sort_order`, then name).
    pub folder_titles: Vec<String>,
}

fn parse(json: &str) -> Result<KeyboardSnapshot, SnapshotError> {
    // The serde error is discarded on purpose: its message can echo document
    // bytes, and nothing from a snapshot may leak into extension logs.
    let snapshot: KeyboardSnapshot =
        serde_json::from_str(json).map_err(|_| SnapshotError::Malformed)?;
    snapshot
        .validate()
        .map_err(|error| SnapshotError::Invalid {
            field: error.field.to_string(),
        })?;
    Ok(snapshot)
}

fn sorted_folder_titles(snapshot: &KeyboardSnapshot) -> Vec<String> {
    let mut folders: Vec<_> = snapshot.folder_metadata.iter().collect();
    folders.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.name.cmp(&b.name))
    });
    folders.into_iter().map(|f| f.name.clone()).collect()
}

fn count(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

/// Parses and validates a snapshot document, returning the overview an
/// extension needs before it trusts the file (external input rule: parse,
/// then validate, then use).
#[uniffi::export]
pub fn parse_snapshot(json: String) -> Result<SnapshotOverview, SnapshotError> {
    let snapshot = parse(&json)?;
    Ok(SnapshotOverview {
        snapshot_version: snapshot.snapshot_version,
        generated_at: snapshot.generated_at,
        device_id: snapshot.device_id.clone(),
        entry_total: count(snapshot.snippets.len()),
        recent_total: count(snapshot.recent_ids.len()),
        favorite_total: count(snapshot.favorite_ids.len()),
        folder_titles: sorted_folder_titles(&snapshot),
    })
}

/// Case-insensitive substring filter over folder titles in display order;
/// an empty query returns every title.
#[uniffi::export]
pub fn filter_folder_titles(json: String, query: String) -> Result<Vec<String>, SnapshotError> {
    let snapshot = parse(&json)?;
    let needle = query.to_lowercase();
    Ok(sorted_folder_titles(&snapshot)
        .into_iter()
        .filter(|title| title.to_lowercase().contains(&needle))
        .collect())
}

/// One row the keyboard list renders.
///
/// Red line: for a sensitive snippet every plaintext field is empty —
/// `title` is `""`, `trigger`/`folder_id` are `None`, `snippet_type` is
/// `""` — because the snapshot itself carries nothing but the id and an
/// opaque ciphertext envelope for those entries. `is_recent` mirrors the
/// snapshot's `recent_ids` usage metadata, which is not content.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SnapshotEntry {
    pub id: String,
    pub title: String,
    /// Controlled TEXT value of the core `SnippetType`; unknown values must
    /// degrade to a plain-text presentation on the platform side.
    pub snippet_type: String,
    pub trigger: Option<String>,
    pub folder_id: Option<String>,
    pub is_favorite: bool,
    pub is_recent: bool,
    pub is_sensitive: bool,
}

/// A folder the keyboard can offer as a category chip.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SnapshotFolder {
    pub id: String,
    pub name: String,
}

fn entry_from(snippet: &SnapshotSnippet, recent: &HashSet<&str>) -> SnapshotEntry {
    match snippet {
        SnapshotSnippet::Normal {
            id,
            title,
            snippet_type,
            trigger,
            folder_id,
            is_favorite,
            ..
        } => SnapshotEntry {
            id: id.clone(),
            title: title.clone(),
            snippet_type: snippet_type.clone(),
            trigger: trigger.clone(),
            folder_id: folder_id.clone(),
            is_favorite: *is_favorite,
            is_recent: recent.contains(id.as_str()),
            is_sensitive: false,
        },
        SnapshotSnippet::Sensitive { id, .. } => SnapshotEntry {
            id: id.clone(),
            title: String::new(),
            snippet_type: String::new(),
            trigger: None,
            folder_id: None,
            is_favorite: false,
            is_recent: recent.contains(id.as_str()),
            is_sensitive: true,
        },
    }
}

fn snippet_id(snippet: &SnapshotSnippet) -> &str {
    match snippet {
        SnapshotSnippet::Normal { id, .. } | SnapshotSnippet::Sensitive { id, .. } => id,
    }
}

/// Entries in default display order: snippets named by `recent_ids` first
/// (most recent first), then the rest in file order. The keyboard renders
/// this list directly, so the ordering rule lives here, not in Swift.
fn ordered_entries(snapshot: &KeyboardSnapshot) -> Vec<SnapshotEntry> {
    let recent: HashSet<&str> = snapshot.recent_ids.iter().map(String::as_str).collect();
    let mut rows: Vec<SnapshotEntry> = Vec::with_capacity(snapshot.snippets.len());
    let mut seen: HashSet<&str> = HashSet::new();
    for id in &snapshot.recent_ids {
        if let Some(snippet) = snapshot
            .snippets
            .iter()
            .find(|s| snippet_id(s) == id)
            .filter(|s| seen.insert(snippet_id(s)))
        {
            rows.push(entry_from(snippet, &recent));
        }
    }
    for snippet in &snapshot.snippets {
        if seen.insert(snippet_id(snippet)) {
            rows.push(entry_from(snippet, &recent));
        }
    }
    rows
}

/// Parses a snapshot and returns its entries in default display order
/// (recent first, then file order). Sensitive snippets surface as locked
/// rows: id and flags only, no plaintext.
#[uniffi::export]
pub fn snapshot_entries(json: String) -> Result<Vec<SnapshotEntry>, SnapshotError> {
    Ok(ordered_entries(&parse(&json)?))
}

/// Body text of one normal snippet, for insertion. `None` when the id is
/// unknown or names a sensitive snippet (its body exists only as ciphertext
/// the extension cannot decrypt).
#[uniffi::export]
pub fn entry_body(json: String, id: String) -> Result<Option<String>, SnapshotError> {
    let snapshot = parse(&json)?;
    Ok(snapshot.snippets.iter().find_map(|snippet| match snippet {
        SnapshotSnippet::Normal {
            id: entry_id, body, ..
        } if *entry_id == id => Some(body.clone()),
        _ => None,
    }))
}

/// Case-insensitive substring search over title, trigger, and body of
/// normal entries, in default display order. Sensitive entries are never
/// searchable (they have no plaintext to match); an empty query returns
/// the full default list, locked rows included.
#[uniffi::export]
pub fn filter_entries(json: String, query: String) -> Result<Vec<SnapshotEntry>, SnapshotError> {
    let snapshot = parse(&json)?;
    if query.is_empty() {
        return Ok(ordered_entries(&snapshot));
    }
    let needle = query.to_lowercase();
    let matches: HashSet<&str> = snapshot
        .snippets
        .iter()
        .filter_map(|snippet| match snippet {
            SnapshotSnippet::Normal {
                id,
                title,
                trigger,
                body,
                ..
            } => {
                let in_trigger = trigger
                    .as_ref()
                    .is_some_and(|t| t.to_lowercase().contains(&needle));
                if title.to_lowercase().contains(&needle)
                    || in_trigger
                    || body.to_lowercase().contains(&needle)
                {
                    Some(id.as_str())
                } else {
                    None
                }
            }
            SnapshotSnippet::Sensitive { .. } => None,
        })
        .collect();
    Ok(ordered_entries(&snapshot)
        .into_iter()
        .filter(|entry| matches.contains(entry.id.as_str()))
        .collect())
}

/// Rows the home-screen widget renders: favorites first (the
/// snapshot's `favorite_ids` display order), then recents (`recent_ids`
/// order), then remaining entries in file order, truncated to `limit`.
///
/// Red line: sensitive snippets are excluded entirely — not even a locked
/// placeholder row — because the widget is an always-visible glance
/// surface, stricter than the actively-invoked keyboard list.
#[uniffi::export]
pub fn widget_entries(json: String, limit: u32) -> Result<Vec<SnapshotEntry>, SnapshotError> {
    let snapshot = parse(&json)?;
    let recent: HashSet<&str> = snapshot.recent_ids.iter().map(String::as_str).collect();
    let limit = limit as usize;
    let mut rows: Vec<SnapshotEntry> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    // Ids in favorite_ids/recent_ids that name no snippet (stale metadata)
    // drop out in the lookup, mirroring `ordered_entries`.
    let named = snapshot
        .favorite_ids
        .iter()
        .chain(snapshot.recent_ids.iter())
        .filter_map(|id| snapshot.snippets.iter().find(|s| snippet_id(s) == id));
    for snippet in named.chain(snapshot.snippets.iter()) {
        if rows.len() == limit {
            break;
        }
        if matches!(snippet, SnapshotSnippet::Sensitive { .. }) {
            continue;
        }
        if !seen.insert(snippet_id(snippet)) {
            continue;
        }
        rows.push(entry_from(snippet, &recent));
    }
    Ok(rows)
}

/// Synthesizes a plain single-line field per `{{variable}}` so the shared
/// template engine can preview/render a keyboard fill without saved field
/// definitions (the snapshot carries none). Rules stay single-sourced in
/// `typvia-template`; this layer only adapts shapes.
fn synthesized_fields(body: &str) -> Result<Vec<typvia_core::model::TemplateField>, SnapshotError> {
    let names = typvia_template::variables(body).map_err(|e| SnapshotError::Invalid {
        field: e.to_string(),
    })?;
    Ok(names
        .into_iter()
        .enumerate()
        .map(|(index, name)| typvia_core::model::TemplateField {
            id: String::new(),
            snippet_id: String::new(),
            name: name.clone(),
            label: name,
            field_type: typvia_core::model::TemplateFieldType::SingleLineText,
            default_value: None,
            options: Vec::new(),
            validation: None,
            is_required: false,
            sort_order: index as i32,
            platform_overrides: None,
        })
        .collect())
}

/// Body of a normal entry, or `Invalid` when the id is unknown or sensitive
/// (a sensitive entry has no plaintext to fill — same red line as search).
fn normal_body(json: &str, id: &str) -> Result<String, SnapshotError> {
    let snapshot = parse(json)?;
    snapshot
        .snippets
        .iter()
        .find_map(|snippet| match snippet {
            SnapshotSnippet::Normal {
                id: entry_id, body, ..
            } if *entry_id == id => Some(body.clone()),
            _ => None,
        })
        .ok_or(SnapshotError::Invalid {
            field: "no such fillable entry".to_string(),
        })
}

/// Distinct `{{variable}}` names of one normal entry's body, in first-seen
/// order (the keyboard template fill's field list). Empty when the body has
/// no variables; `Invalid` for an unknown or sensitive id.
#[uniffi::export]
pub fn template_variables(json: String, id: String) -> Result<Vec<String>, SnapshotError> {
    let body = normal_body(&json, &id)?;
    typvia_template::variables(&body).map_err(|e| SnapshotError::Invalid {
        field: e.to_string(),
    })
}

/// Lenient fill preview: filled values win, an unfilled variable shows a
/// `‹name›` placeholder. Never fails on missing values.
#[uniffi::export]
pub fn template_fill_preview(
    json: String,
    id: String,
    values: std::collections::HashMap<String, String>,
) -> Result<String, SnapshotError> {
    let body = normal_body(&json, &id)?;
    let fields = synthesized_fields(&body)?;
    typvia_template::preview(&body, &fields, &values).map_err(|e| SnapshotError::Invalid {
        field: e.to_string(),
    })
}

/// Final fill for insertion: filled values substituted, unfilled variables
/// become empty text (no placeholder marks leave the keyboard).
#[uniffi::export]
pub fn template_fill_render(
    json: String,
    id: String,
    values: std::collections::HashMap<String, String>,
) -> Result<String, SnapshotError> {
    let body = normal_body(&json, &id)?;
    let fields = synthesized_fields(&body)?;
    let rendered =
        typvia_template::render(&body, &fields, &values).map_err(|e| SnapshotError::Invalid {
            field: e.to_string(),
        })?;
    Ok(rendered
        .segments()
        .iter()
        .map(|segment| match segment {
            typvia_template::RenderSegment::Text(text) => text.as_str(),
            // Synthesized fields are never secret_ref; keep the arm honest
            // anyway: a reference renders as nothing, never a plaintext.
            typvia_template::RenderSegment::Secret { .. } => "",
        })
        .collect())
}

/// Folders that at least one snapshot entry belongs to, in display order
/// (`sort_order`, then name). Empty folders never earn a chip.
#[uniffi::export]
pub fn snapshot_folders(json: String) -> Result<Vec<SnapshotFolder>, SnapshotError> {
    let snapshot = parse(&json)?;
    let used: HashSet<&str> = snapshot
        .snippets
        .iter()
        .filter_map(|snippet| match snippet {
            SnapshotSnippet::Normal {
                folder_id: Some(folder_id),
                ..
            } => Some(folder_id.as_str()),
            _ => None,
        })
        .collect();
    let mut folders: Vec<_> = snapshot
        .folder_metadata
        .iter()
        .filter(|f| used.contains(f.id.as_str()))
        .collect();
    folders.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(folders
        .into_iter()
        .map(|f| SnapshotFolder {
            id: f.id.clone(),
            name: f.name.clone(),
        })
        .collect())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use typvia_core::model::{FolderMetadata, SNAPSHOT_VERSION};

    fn snapshot() -> KeyboardSnapshot {
        KeyboardSnapshot {
            snapshot_version: SNAPSHOT_VERSION,
            generated_at: 1_700_000_000_000,
            device_id: "device-1".to_string(),
            snippets: vec![],
            recent_ids: vec!["s2".to_string(), "s1".to_string()],
            favorite_ids: vec!["s3".to_string()],
            folder_metadata: vec![
                FolderMetadata {
                    id: "f2".to_string(),
                    name: "Shell".to_string(),
                    sort_order: 1,
                },
                FolderMetadata {
                    id: "f1".to_string(),
                    name: "Email".to_string(),
                    sort_order: 0,
                },
            ],
        }
    }

    fn snapshot_json() -> String {
        serde_json::to_string(&snapshot()).unwrap()
    }

    #[test]
    fn parse_reports_counts_and_display_ordered_folder_titles() {
        let overview = parse_snapshot(snapshot_json()).unwrap();
        assert_eq!(overview.snapshot_version, SNAPSHOT_VERSION);
        assert_eq!(overview.generated_at, 1_700_000_000_000);
        assert_eq!(overview.device_id, "device-1");
        assert_eq!(overview.recent_total, 2);
        assert_eq!(overview.favorite_total, 1);
        assert_eq!(overview.folder_titles, vec!["Email", "Shell"]);
        // The counts answer different questions and this fixture keeps them
        // apart on purpose: it names two recent ids while carrying no
        // entries. Taking `recent_total` for the denominator is how a bench
        // showing three rows once printed "3 / 1".
        assert_eq!(overview.entry_total, 0);
    }

    /// The denominator an extension prints is the number of entries it was
    /// actually handed — nothing else in the document is a stand-in for it.
    #[test]
    fn the_entry_total_counts_the_entries_the_snapshot_carries() {
        let mut snapshot = snapshot();
        snapshot.snippets = vec![
            SnapshotSnippet::Normal {
                id: "s1".to_string(),
                title: "Signature".to_string(),
                snippet_type: "text".to_string(),
                trigger: Some(";sig".to_string()),
                trigger_mode: None,
                folder_id: None,
                is_favorite: false,
                body: "Best,\nR".to_string(),
            },
            SnapshotSnippet::Sensitive {
                id: "s2".to_string(),
                encrypted_metadata: vec![1, 2, 3],
            },
        ];

        let overview = parse_snapshot(serde_json::to_string(&snapshot).unwrap()).unwrap();

        // A locked entry is still an entry: the bench lists it and offers to
        // fetch it, so leaving it out of the count would understate what the
        // keyboard can reach.
        assert_eq!(overview.entry_total, 2);
    }

    #[test]
    fn malformed_json_is_rejected_without_echoing_the_document() {
        // Deliberately fake marker: proves the document body never reaches
        // the error message, whatever it contains.
        let error = parse_snapshot(r#"{"oops": "AKIA_FAKE_MARKER""#.to_string()).unwrap_err();
        assert_eq!(error, SnapshotError::Malformed);
        assert!(!error.to_string().contains("AKIA_FAKE_MARKER"));
    }

    #[test]
    fn newer_snapshot_version_is_rejected_with_the_field_name_only() {
        let mut s = snapshot();
        s.snapshot_version = SNAPSHOT_VERSION + 1;
        let json = serde_json::to_string(&s).unwrap();

        let error = parse_snapshot(json).unwrap_err();
        assert_eq!(
            error,
            SnapshotError::Invalid {
                field: "snapshot_version".to_string(),
            }
        );
    }

    #[test]
    fn filter_matches_titles_case_insensitively_in_display_order() {
        let hits = filter_folder_titles(snapshot_json(), "sHe".to_string()).unwrap();
        assert_eq!(hits, vec!["Shell"]);
    }

    #[test]
    fn empty_query_returns_every_title_in_display_order() {
        let hits = filter_folder_titles(snapshot_json(), String::new()).unwrap();
        assert_eq!(hits, vec!["Email", "Shell"]);
    }

    fn normal(id: &str, title: &str, trigger: Option<&str>, body: &str) -> SnapshotSnippet {
        SnapshotSnippet::Normal {
            id: id.to_string(),
            title: title.to_string(),
            snippet_type: "command".to_string(),
            trigger: trigger.map(str::to_string),
            trigger_mode: trigger.map(|_| "delimiter".to_string()),
            folder_id: Some("f1".to_string()),
            is_favorite: id == "s2",
            body: body.to_string(),
        }
    }

    /// Fixture with normal + sensitive entries. The sensitive envelope bytes
    /// spell a deliberately fake marker so leak assertions can grep every
    /// output for it.
    fn entry_snapshot() -> KeyboardSnapshot {
        let mut s = snapshot();
        s.snippets = vec![
            normal("s1", "Docker logs", Some(":dlog"), "docker logs -f app"),
            normal("s2", "Standup", None, "Yesterday I shipped"),
            SnapshotSnippet::Sensitive {
                id: "s9".to_string(),
                encrypted_metadata: b"AKIA_FAKE_SECRET".to_vec(),
            },
        ];
        // s2 used most recently; "ghost" proves stale recent ids are skipped.
        s.recent_ids = vec!["s2".to_string(), "ghost".to_string()];
        s
    }

    fn entry_json() -> String {
        serde_json::to_string(&entry_snapshot()).unwrap()
    }

    fn template_snapshot_json() -> String {
        let mut s = entry_snapshot();
        s.snippets.push(normal(
            "t1",
            "PR review",
            None,
            "Hi {{name}}, review {{pr}} for {{name}}.",
        ));
        serde_json::to_string(&s).unwrap()
    }

    #[test]
    fn template_variables_lists_distinct_names_in_first_seen_order() {
        let names = template_variables(template_snapshot_json(), "t1".to_string()).unwrap();
        assert_eq!(names, vec!["name", "pr"]);
    }

    #[test]
    fn template_fill_preview_substitutes_filled_and_marks_unfilled() {
        let mut values = std::collections::HashMap::new();
        values.insert("name".to_string(), "Lin".to_string());
        let out =
            template_fill_preview(template_snapshot_json(), "t1".to_string(), values).unwrap();
        assert_eq!(out, "Hi Lin, review ‹pr› for Lin.");
    }

    #[test]
    fn template_fill_render_substitutes_filled_and_blanks_unfilled() {
        let mut values = std::collections::HashMap::new();
        values.insert("name".to_string(), "Lin".to_string());
        let out = template_fill_render(template_snapshot_json(), "t1".to_string(), values).unwrap();
        assert_eq!(out, "Hi Lin, review  for Lin.");
    }

    #[test]
    fn template_fill_refuses_sensitive_and_unknown_ids_without_content() {
        // Red line: a sensitive id has no fillable plaintext; the error names
        // no bytes from the document.
        for id in ["s9", "missing"] {
            let error = template_variables(template_snapshot_json(), id.to_string()).unwrap_err();
            assert!(!error.to_string().contains("AKIA_FAKE_SECRET"));
            assert!(matches!(error, SnapshotError::Invalid { .. }));
        }
    }

    #[test]
    fn entries_come_recent_first_then_file_order() {
        let ids: Vec<String> = snapshot_entries(entry_json())
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(ids, vec!["s2", "s1", "s9"]);
    }

    #[test]
    fn a_normal_entry_carries_its_plaintext_fields_and_flags() {
        let entries = snapshot_entries(entry_json()).unwrap();
        let s2 = entries.iter().find(|e| e.id == "s2").unwrap();
        assert_eq!(s2.title, "Standup");
        assert_eq!(s2.snippet_type, "command");
        assert_eq!(s2.trigger, None);
        assert_eq!(s2.folder_id, Some("f1".to_string()));
        assert!(s2.is_favorite);
        assert!(s2.is_recent);
        assert!(!s2.is_sensitive);
        let s1 = entries.iter().find(|e| e.id == "s1").unwrap();
        assert_eq!(s1.trigger, Some(":dlog".to_string()));
        assert!(!s1.is_recent);
    }

    #[test]
    fn entry_body_returns_the_normal_body_and_none_for_unknown_ids() {
        assert_eq!(
            entry_body(entry_json(), "s1".to_string()).unwrap(),
            Some("docker logs -f app".to_string())
        );
        assert_eq!(entry_body(entry_json(), "nope".to_string()).unwrap(), None);
    }

    #[test]
    fn filter_matches_title_trigger_and_body_case_insensitively() {
        let by_title: Vec<String> = filter_entries(entry_json(), "STAND".to_string())
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(by_title, vec!["s2"]);
        let by_trigger: Vec<String> = filter_entries(entry_json(), "dLoG".to_string())
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(by_trigger, vec!["s1"]);
        let by_body: Vec<String> = filter_entries(entry_json(), "yesterday".to_string())
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(by_body, vec!["s2"]);
    }

    #[test]
    fn empty_query_returns_the_default_list_locked_rows_included() {
        let entries = filter_entries(entry_json(), String::new()).unwrap();
        assert_eq!(entries, snapshot_entries(entry_json()).unwrap());
        assert!(entries.iter().any(|e| e.is_sensitive));
    }

    #[test]
    fn only_folders_with_entries_earn_a_chip_in_display_order() {
        let folders = snapshot_folders(entry_json()).unwrap();
        // f2 ("Shell") exists in metadata but holds no entries.
        assert_eq!(
            folders,
            vec![SnapshotFolder {
                id: "f1".to_string(),
                name: "Email".to_string(),
            }]
        );
    }

    /// Fixture for the widget surface: four normal entries plus a sensitive
    /// one that is favorite-listed AND recent-listed, so ordering tests and
    /// the red-line test share one document.
    fn widget_snapshot() -> KeyboardSnapshot {
        let mut s = snapshot();
        s.snippets = vec![
            normal("s1", "Docker logs", Some(":dlog"), "docker logs -f app"),
            normal("s2", "Standup", None, "Yesterday I shipped"),
            normal("s3", "Address", None, "1 Example Way"),
            normal("s4", "Sign-off", None, "Best regards"),
            SnapshotSnippet::Sensitive {
                id: "s9".to_string(),
                encrypted_metadata: b"AKIA_FAKE_SECRET".to_vec(),
            },
        ];
        // The sensitive entry sits first in both lists; "ghost" proves stale
        // ids are skipped; s3 is favorite AND recent (dedup case).
        s.favorite_ids = vec!["s9".to_string(), "s3".to_string()];
        s.recent_ids = vec![
            "s9".to_string(),
            "s2".to_string(),
            "s3".to_string(),
            "ghost".to_string(),
        ];
        s
    }

    fn widget_json() -> String {
        serde_json::to_string(&widget_snapshot()).unwrap()
    }

    #[test]
    fn widget_rows_come_favorites_then_recents_then_file_order_deduped() {
        let ids: Vec<String> = widget_entries(widget_json(), 8)
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(ids, vec!["s3", "s2", "s1", "s4"]);
    }

    #[test]
    fn widget_rows_truncate_at_the_requested_limit() {
        let ids: Vec<String> = widget_entries(widget_json(), 2)
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(ids, vec!["s3", "s2"]);
        assert_eq!(widget_entries(widget_json(), 0).unwrap(), vec![]);
    }

    /// Red line: the widget surface carries no sensitive entry
    /// in any form — no locked row, no id, no envelope bytes — even when the
    /// sensitive entry leads both the favorite and recent lists.
    #[test]
    fn widget_rows_exclude_sensitive_entries_entirely() {
        let rows = widget_entries(widget_json(), 8).unwrap();
        assert!(rows.iter().all(|e| !e.is_sensitive));
        assert!(rows.iter().all(|e| e.id != "s9"));
        assert!(!format!("{rows:?}").contains("AKIA_FAKE_SECRET"));
    }

    /// Red line: across every exported function, a sensitive
    /// entry never yields a title, trigger, body, folder, or its envelope
    /// bytes — only its id and flags.
    #[test]
    fn a_sensitive_entry_leaks_nothing_through_any_exported_function() {
        let json = entry_json();
        assert!(json.contains("s9"), "fixture must carry the sensitive id");

        let locked = snapshot_entries(json.clone())
            .unwrap()
            .into_iter()
            .find(|e| e.id == "s9")
            .unwrap();
        assert!(locked.is_sensitive);
        assert_eq!(locked.title, "");
        assert_eq!(locked.snippet_type, "");
        assert_eq!(locked.trigger, None);
        assert_eq!(locked.folder_id, None);
        assert!(!locked.is_favorite);

        assert_eq!(entry_body(json.clone(), "s9".to_string()).unwrap(), None);

        // Searching for the envelope marker (or any text) never surfaces
        // the sensitive entry.
        for query in ["AKIA_FAKE_SECRET", "secret", "s9", "a"] {
            let hits = filter_entries(json.clone(), query.to_string()).unwrap();
            assert!(
                hits.iter().all(|e| !e.is_sensitive),
                "query {query:?} surfaced a sensitive entry"
            );
        }

        // The overview and folder surfaces expose nothing entry-scoped.
        let overview = parse_snapshot(json.clone()).unwrap();
        assert!(!format!("{overview:?}").contains("AKIA_FAKE_SECRET"));
        let folders = snapshot_folders(json).unwrap();
        assert!(!format!("{folders:?}").contains("AKIA_FAKE_SECRET"));
    }
}

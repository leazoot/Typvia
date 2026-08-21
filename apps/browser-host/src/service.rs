// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Query layer over the snapshot view: tiered deterministic search, list
//! surfaces, and render.
//!
//! Ranking mirrors the keyboard surfaces' semantics: trigger prefix beats
//! title prefix beats title substring beats body substring; ties break by
//! favorite, then recency rank, then title, then id — fully deterministic
//! for identical snapshots.

use std::collections::HashMap;

use serde_json::Value;

use crate::protocol::{
    ErrorResponse, HelloResponse, RenderResponse, Request, ResultEntry, ResultsResponse,
};
use crate::store::{Entry, SnapshotStore, StoreError, View};

/// Default and maximum result-page sizes (panel parity).
const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 200;
/// Preview cap in characters (single list line).
const PREVIEW_CHARS: usize = 120;
/// Wire protocol version answered in `hello`.
const PROTOCOL_VERSION: u32 = 1;

/// Handles one request against the current snapshot. Always answers — every
/// failure path is a stable error code, never a panic or a dropped frame.
pub fn handle(request: &Request, store: &mut SnapshotStore) -> Value {
    match request {
        Request::Hello => hello(store),
        Request::Search { query, limit } => {
            with_view(store, |view| results(search(view, query, size(*limit))))
        }
        Request::ListRecent { limit } => {
            with_view(store, |view| results(list_recent(view, size(*limit))))
        }
        Request::ListFavorites { limit } => {
            with_view(store, |view| results(list_favorites(view, size(*limit))))
        }
        Request::Render {
            snippet_id,
            variables,
        } => with_view(store, |view| render(view, snippet_id, variables)),
    }
}

fn size(limit: Option<u32>) -> usize {
    limit.map_or(DEFAULT_LIMIT, |value| (value as usize).min(MAX_LIMIT))
}

fn hello(store: &mut SnapshotStore) -> Value {
    let (present, generated_at, count) = match store.view() {
        Ok(view) => (true, Some(view.generated_at), Some(view.entries.len())),
        Err(_) => (false, None, None),
    };
    json(&HelloResponse {
        ok: true,
        protocol_version: PROTOCOL_VERSION,
        snapshot_present: present,
        generated_at,
        snippet_count: count,
    })
}

fn with_view(store: &mut SnapshotStore, serve: impl FnOnce(&View) -> Value) -> Value {
    match store.view() {
        Ok(view) => serve(view),
        Err(StoreError::Unavailable) => json(&ErrorResponse::code("snapshot_unavailable")),
        Err(StoreError::Invalid) => json(&ErrorResponse::code("snapshot_invalid")),
    }
}

fn results(entries: Vec<ResultEntry>) -> Value {
    json(&ResultsResponse {
        ok: true,
        results: entries,
    })
}

/// Tiered match: lower is better; `None` filters the entry out.
fn tier(entry: &Entry, query: &str) -> Option<u8> {
    let title = entry.title.to_lowercase();
    let trigger = entry.trigger.as_deref().unwrap_or("").to_lowercase();
    if !trigger.is_empty() && trigger.starts_with(query) {
        return Some(0);
    }
    if title.starts_with(query) {
        return Some(1);
    }
    if title.contains(query) {
        return Some(2);
    }
    if entry.body.to_lowercase().contains(query) {
        return Some(3);
    }
    None
}

fn recency_rank(view: &View, id: &str) -> usize {
    view.recent_ids
        .iter()
        .position(|recent| recent == id)
        .unwrap_or(usize::MAX)
}

fn search(view: &View, query: &str, limit: usize) -> Vec<ResultEntry> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        // Empty query shows the recent surface (panel parity).
        return list_recent(view, limit);
    }
    let mut hits: Vec<(u8, &Entry)> = view
        .entries
        .iter()
        .filter_map(|entry| tier(entry, &query).map(|tier| (tier, entry)))
        .collect();
    hits.sort_by(|(tier_a, a), (tier_b, b)| {
        tier_a
            .cmp(tier_b)
            .then_with(|| b.is_favorite.cmp(&a.is_favorite))
            .then_with(|| recency_rank(view, &a.id).cmp(&recency_rank(view, &b.id)))
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
    hits.into_iter()
        .take(limit)
        .map(|(_, entry)| result_entry(entry))
        .collect()
}

fn list_recent(view: &View, limit: usize) -> Vec<ResultEntry> {
    let mut ordered: Vec<&Entry> = Vec::new();
    for id in &view.recent_ids {
        if let Some(entry) = view.entries.iter().find(|entry| &entry.id == id) {
            ordered.push(entry);
        }
    }
    for entry in &view.entries {
        if !view.recent_ids.contains(&entry.id) {
            ordered.push(entry);
        }
    }
    ordered.into_iter().take(limit).map(result_entry).collect()
}

fn list_favorites(view: &View, limit: usize) -> Vec<ResultEntry> {
    let mut ordered: Vec<&Entry> = Vec::new();
    for id in &view.favorite_ids {
        if let Some(entry) = view.entries.iter().find(|entry| &entry.id == id) {
            ordered.push(entry);
        }
    }
    for entry in view.entries.iter().filter(|entry| entry.is_favorite) {
        if !view.favorite_ids.contains(&entry.id) {
            ordered.push(entry);
        }
    }
    ordered.into_iter().take(limit).map(result_entry).collect()
}

fn result_entry(entry: &Entry) -> ResultEntry {
    ResultEntry {
        id: entry.id.clone(),
        title: entry.title.clone(),
        snippet_type: entry.snippet_type.clone(),
        trigger: entry.trigger.clone(),
        is_favorite: entry.is_favorite,
        preview: preview_of(&entry.body),
        variables: variables_of(&entry.body),
    }
}

fn preview_of(body: &str) -> String {
    let line = body.lines().next().unwrap_or("");
    let mut preview: String = line.chars().take(PREVIEW_CHARS).collect();
    if line.chars().count() > PREVIEW_CHARS || body.lines().count() > 1 {
        preview.push('…');
    }
    preview
}

/// Variable names referenced by `body`; a body that does not parse as a
/// template is served as plain text (no variables), so an authoring typo
/// never blocks plain insertion of what the user sees.
fn variables_of(body: &str) -> Vec<String> {
    typvia_template::variables(body).unwrap_or_default()
}

/// Renders the final text for insertion. Substitution runs over the shared
/// template parser; field definitions (defaults, secret refs) live in the
/// main database and are not part of the snapshot, so every variable is a
/// required plain-text input here (a known v1 limitation).
fn render(view: &View, snippet_id: &str, variables: &HashMap<String, String>) -> Value {
    let Some(entry) = view.entries.iter().find(|entry| entry.id == snippet_id) else {
        return json(&ErrorResponse::code("unknown_snippet"));
    };
    let Ok(segments) = typvia_template::parse(&entry.body) else {
        // Not a well-formed template: insert the body as the user sees it.
        return json(&RenderResponse {
            ok: true,
            text: entry.body.clone(),
        });
    };
    let mut text = String::new();
    for segment in segments {
        match segment {
            typvia_template::Segment::Literal(literal) => text.push_str(&literal),
            typvia_template::Segment::Variable(name) => match variables.get(&name) {
                Some(value) if !value.is_empty() => text.push_str(value),
                _ => {
                    return json(&ErrorResponse {
                        ok: false,
                        code: "missing_variable",
                        field: Some(name),
                    });
                }
            },
        }
    }
    json(&RenderResponse { ok: true, text })
}

fn json<T: serde::Serialize>(value: &T) -> Value {
    // Responses are built from owned data; serialization cannot fail for
    // these shapes, and a defensive fallback keeps the stream answered.
    serde_json::to_value(value)
        .unwrap_or_else(|_| serde_json::json!({ "ok": false, "code": "internal" }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn entry(id: &str, title: &str, trigger: Option<&str>, favorite: bool, body: &str) -> Entry {
        Entry {
            id: id.to_string(),
            title: title.to_string(),
            snippet_type: "text".to_string(),
            trigger: trigger.map(str::to_string),
            is_favorite: favorite,
            body: body.to_string(),
        }
    }

    fn view() -> View {
        View {
            generated_at: 1,
            entries: vec![
                entry("s1", "Docker logs", Some(";dlog"), false, "docker logs -f"),
                entry("s2", "Log rotate", None, true, "logrotate --force"),
                entry("s3", "Greeting", None, false, "hello {{name}}, welcome"),
                entry("s4", "Notes", None, false, "first line\nsecond line"),
            ],
            recent_ids: vec!["s2".to_string(), "s1".to_string()],
            favorite_ids: vec!["s2".to_string()],
        }
    }

    #[test]
    fn search_ranks_trigger_prefix_over_title_matches() {
        let hits = search(&view(), "log", 50);
        // ";dlog" does not start with "log": s1 matches by title-contains
        // (tier 2); "Log rotate" is a title prefix (tier 1).
        assert_eq!(
            hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
            vec!["s2", "s1"]
        );

        let trigger_hits = search(&view(), ";d", 50);
        assert_eq!(trigger_hits[0].id, "s1");
    }

    #[test]
    fn empty_query_serves_the_recent_surface() {
        let hits = search(&view(), "  ", 50);
        assert_eq!(hits[0].id, "s2");
        assert_eq!(hits[1].id, "s1");
        assert_eq!(hits.len(), 4);
    }

    #[test]
    fn search_respects_the_limit() {
        assert_eq!(search(&view(), "l", 1).len(), 1);
    }

    #[test]
    fn previews_collapse_to_one_capped_line() {
        let hits = search(&view(), "first line", 50);
        assert_eq!(hits[0].preview, "first line…");
    }

    #[test]
    fn template_variables_surface_on_results() {
        let hits = search(&view(), "greeting", 50);
        assert_eq!(hits[0].variables, vec!["name".to_string()]);
    }

    #[test]
    fn render_substitutes_supplied_variables() {
        let mut variables = HashMap::new();
        variables.insert("name".to_string(), "Ada".to_string());
        let value = render(&view(), "s3", &variables);
        assert_eq!(value["ok"], true);
        assert_eq!(value["text"], "hello Ada, welcome");
    }

    #[test]
    fn render_reports_the_missing_variable_by_name_only() {
        let value = render(&view(), "s3", &HashMap::new());
        assert_eq!(value["ok"], false);
        assert_eq!(value["code"], "missing_variable");
        assert_eq!(value["field"], "name");
        // The error carries no snippet content.
        assert!(!value.to_string().contains("welcome"));
    }

    #[test]
    fn render_of_a_plain_snippet_returns_its_body() {
        let value = render(&view(), "s1", &HashMap::new());
        assert_eq!(value["text"], "docker logs -f");
    }

    #[test]
    fn render_of_an_unknown_id_is_a_stable_code() {
        let value = render(&view(), "nope", &HashMap::new());
        assert_eq!(value["code"], "unknown_snippet");
    }

    #[test]
    fn favorites_list_follows_display_order() {
        let hits = list_favorites(&view(), 50);
        assert_eq!(hits[0].id, "s2");
    }
}

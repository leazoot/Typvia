// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Repository CRUD behavior: snippets, folders, tags, batch operations,
//! usage tracking, trigger conflicts, and duplicate detection.

#![allow(clippy::unwrap_used)]

use rusqlite::Connection;
use typvia_core::db::{migrate_to_latest, open_in_memory};
use typvia_core::model::{
    AppRule, AppRuleType, Folder, Platform, SecurityLevel, Snippet, SnippetContent, SnippetType,
    Tag, TriggerMode,
};
use typvia_core::repo::{
    AppRuleRepo, FolderRepo, ListOrder, ListScope, RepoError, SnippetRepo, TagRepo, new_id,
};

fn fresh_db() -> Connection {
    let mut conn = open_in_memory().unwrap();
    migrate_to_latest(&mut conn).unwrap();
    conn
}

fn snippet(title: &str, body: &str) -> Snippet {
    Snippet {
        id: new_id(),
        workspace_id: "w1".to_string(),
        title: title.to_string(),
        content: SnippetContent::Plaintext(body.to_string()),
        snippet_type: SnippetType::Command,
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
        created_at: 1_000,
        updated_at: 1_000,
        last_used_at: None,
        usage_count: 0,
        version: 1,
        deleted_at: None,
        conflict_of: None,
    }
}

fn folder(name: &str, parent_id: Option<&str>) -> Folder {
    Folder {
        id: new_id(),
        parent_id: parent_id.map(str::to_string),
        name: name.to_string(),
        sort_order: 0,
        created_at: 1_000,
        updated_at: 1_000,
    }
}

fn tag(name: &str) -> Tag {
    Tag {
        id: new_id(),
        name: name.to_string(),
        created_at: 1_000,
    }
}

#[test]
fn snippet_insert_and_get_round_trips_every_field() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let mut s = snippet("Docker logs", "docker logs -f app");
    s.trigger = Some(":dlog".to_string());
    s.trigger_mode = Some(TriggerMode::Delimiter);
    s.platform_scope = vec![
        typvia_core::model::Platform::Macos,
        typvia_core::model::Platform::Windows,
    ];
    s.description = Some("Tail app logs".to_string());
    repo.insert(&s).unwrap();

    let loaded = repo.get(&s.id).unwrap().unwrap();
    assert_eq!(loaded, s);
}

#[test]
fn snippet_get_returns_none_for_unknown_id() {
    let conn = fresh_db();
    assert!(SnippetRepo::new(&conn).get("missing").unwrap().is_none());
}

#[test]
fn snippet_insert_rejects_invalid_values() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let mut s = snippet(" ", "body");
    let err = repo.insert(&s).unwrap_err();
    assert!(matches!(err, RepoError::Validation(_)));
    s.title = "ok".to_string();
    s.security_level = SecurityLevel::Sensitive;
    assert!(matches!(
        repo.insert(&s).unwrap_err(),
        RepoError::Validation(_)
    ));
}

#[test]
fn snippet_update_replaces_columns_and_flags_missing_rows() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let mut s = snippet("Title", "body");
    repo.insert(&s).unwrap();

    s.title = "New title".to_string();
    s.updated_at = 2_000;
    repo.update(&s).unwrap();
    assert_eq!(repo.get(&s.id).unwrap().unwrap().title, "New title");

    let ghost = snippet("Ghost", "body");
    assert!(matches!(
        repo.update(&ghost).unwrap_err(),
        RepoError::NotFound
    ));
}

#[test]
fn snippet_delete_removes_the_row() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let s = snippet("Doomed", "body");
    repo.insert(&s).unwrap();
    repo.delete(&s.id).unwrap();
    assert!(repo.get(&s.id).unwrap().is_none());
    assert!(matches!(
        repo.delete(&s.id).unwrap_err(),
        RepoError::NotFound
    ));
}

#[test]
fn snippet_list_orders_by_recency_and_respects_limit_offset() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    for (i, title) in ["a", "b", "c"].iter().enumerate() {
        let mut s = snippet(title, "body");
        s.updated_at = 1_000 + i as i64;
        repo.insert(&s).unwrap();
    }
    let page = repo.list(2, 0).unwrap();
    assert_eq!(
        page.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
        ["c", "b"]
    );
    let rest = repo.list(2, 2).unwrap();
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].title, "a");
}

#[test]
fn snippet_list_by_folder_separates_unfiled_from_filed() {
    let conn = fresh_db();
    let folders = FolderRepo::new(&conn);
    let f = folder("Shell", None);
    folders.insert(&f).unwrap();

    let repo = SnippetRepo::new(&conn);
    let mut filed = snippet("Filed", "body");
    filed.folder_id = Some(f.id.clone());
    repo.insert(&filed).unwrap();
    repo.insert(&snippet("Unfiled", "body")).unwrap();

    let in_folder = repo.list_by_folder(Some(&f.id), 10, 0).unwrap();
    assert_eq!(in_folder.len(), 1);
    assert_eq!(in_folder[0].title, "Filed");

    let unfiled = repo.list_by_folder(None, 10, 0).unwrap();
    assert_eq!(unfiled.len(), 1);
    assert_eq!(unfiled[0].title, "Unfiled");
}

#[test]
fn favorite_pin_enable_toggles_persist() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let s = snippet("Toggles", "body");
    repo.insert(&s).unwrap();

    repo.set_favorite(&s.id, true).unwrap();
    repo.set_pinned(&s.id, true).unwrap();
    repo.set_enabled(&s.id, false).unwrap();

    let loaded = repo.get(&s.id).unwrap().unwrap();
    assert!(loaded.is_favorite);
    assert!(loaded.is_pinned);
    assert!(!loaded.is_enabled);
    assert!(matches!(
        repo.set_favorite("missing", true).unwrap_err(),
        RepoError::NotFound
    ));
}

#[test]
fn batch_move_is_atomic_when_one_id_is_missing() {
    let conn = fresh_db();
    let folders = FolderRepo::new(&conn);
    let f = folder("Target", None);
    folders.insert(&f).unwrap();

    let repo = SnippetRepo::new(&conn);
    let a = snippet("A", "body");
    repo.insert(&a).unwrap();

    let err = repo
        .batch_move(&[a.id.clone(), "missing".to_string()], Some(&f.id))
        .unwrap_err();
    assert!(matches!(err, RepoError::NotFound));
    // The whole batch rolled back: A stays unfiled.
    assert_eq!(repo.get(&a.id).unwrap().unwrap().folder_id, None);

    repo.batch_move(std::slice::from_ref(&a.id), Some(&f.id))
        .unwrap();
    assert_eq!(repo.get(&a.id).unwrap().unwrap().folder_id, Some(f.id));
}

#[test]
fn batch_enable_disable_applies_to_all_ids() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let a = snippet("A", "body");
    let b = snippet("B", "body");
    repo.insert(&a).unwrap();
    repo.insert(&b).unwrap();

    repo.batch_set_enabled(&[a.id.clone(), b.id.clone()], false)
        .unwrap();
    assert!(!repo.get(&a.id).unwrap().unwrap().is_enabled);
    assert!(!repo.get(&b.id).unwrap().unwrap().is_enabled);
}

#[test]
fn batch_tagging_attaches_detaches_and_ignores_duplicates() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let tags = TagRepo::new(&conn);
    let a = snippet("A", "body");
    let b = snippet("B", "body");
    let t = tag("shell");
    repo.insert(&a).unwrap();
    repo.insert(&b).unwrap();
    tags.insert(&t).unwrap();

    let ids = [a.id.clone(), b.id.clone()];
    repo.batch_add_tag(&ids, &t.id).unwrap();
    repo.batch_add_tag(&ids, &t.id).unwrap();
    assert_eq!(repo.tag_ids_of(&a.id).unwrap(), vec![t.id.clone()]);

    repo.batch_remove_tag(std::slice::from_ref(&a.id), &t.id)
        .unwrap();
    assert!(repo.tag_ids_of(&a.id).unwrap().is_empty());
    assert_eq!(repo.tag_ids_of(&b.id).unwrap(), vec![t.id]);
}

#[test]
fn record_usage_bumps_count_and_last_used_only() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let s = snippet("Used", "body");
    repo.insert(&s).unwrap();

    repo.record_usage(&s.id, 5_000).unwrap();
    repo.record_usage(&s.id, 6_000).unwrap();

    let loaded = repo.get(&s.id).unwrap().unwrap();
    assert_eq!(loaded.usage_count, 2);
    assert_eq!(loaded.last_used_at, Some(6_000));
    // Content metadata untouched by usage tracking.
    assert_eq!(loaded.updated_at, s.updated_at);
    assert_eq!(loaded.version, 1);
}

#[test]
fn trigger_conflict_is_found_and_excludes_self() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let mut owner = snippet("Owner", "body");
    owner.trigger = Some(":sig".to_string());
    owner.trigger_mode = Some(TriggerMode::Delimiter);
    repo.insert(&owner).unwrap();

    // Positive: another snippet wanting :sig conflicts with owner.
    assert_eq!(
        repo.find_trigger_conflict(":sig", None).unwrap(),
        Some(owner.id.clone())
    );
    // Negative: the owner editing itself is not a conflict.
    assert_eq!(
        repo.find_trigger_conflict(":sig", Some(&owner.id)).unwrap(),
        None
    );
    // Negative: an unclaimed trigger is free.
    assert_eq!(repo.find_trigger_conflict(":other", None).unwrap(), None);
}

#[test]
fn duplicates_match_on_title_or_plaintext_body() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let original = snippet("Deploy steps", "kubectl rollout restart deploy/app");
    repo.insert(&original).unwrap();

    // Positive: same title.
    let by_title = repo
        .find_duplicates("Deploy steps", Some("different"), None)
        .unwrap();
    assert_eq!(by_title, vec![original.id.clone()]);

    // Positive: same body, different title.
    let by_body = repo
        .find_duplicates("Other", Some("kubectl rollout restart deploy/app"), None)
        .unwrap();
    assert_eq!(by_body, vec![original.id.clone()]);

    // Negative: nothing shared.
    assert!(
        repo.find_duplicates("Other", Some("nothing"), None)
            .unwrap()
            .is_empty()
    );
    // Negative: the row itself is excluded while editing.
    assert!(
        repo.find_duplicates("Deploy steps", None, Some(&original.id))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn folder_nesting_lists_children_in_order() {
    let conn = fresh_db();
    let repo = FolderRepo::new(&conn);
    let root = folder("Root", None);
    repo.insert(&root).unwrap();
    let mut child_b = folder("B", Some(&root.id));
    child_b.sort_order = 2;
    let mut child_a = folder("A", Some(&root.id));
    child_a.sort_order = 1;
    repo.insert(&child_b).unwrap();
    repo.insert(&child_a).unwrap();

    let children = repo.list_children(Some(&root.id)).unwrap();
    assert_eq!(
        children.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        ["A", "B"]
    );
    let top = repo.list_children(None).unwrap();
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].name, "Root");
}

#[test]
fn folder_move_under_own_descendant_is_rejected() {
    let conn = fresh_db();
    let repo = FolderRepo::new(&conn);
    let root = folder("Root", None);
    repo.insert(&root).unwrap();
    let child = folder("Child", Some(&root.id));
    repo.insert(&child).unwrap();

    let mut moved_root = root.clone();
    moved_root.parent_id = Some(child.id.clone());
    let err = repo.update(&moved_root).unwrap_err();
    assert!(matches!(err, RepoError::Conflict(_)));

    // A legal re-parent still works.
    let other = folder("Other", None);
    repo.insert(&other).unwrap();
    let mut moved_child = child.clone();
    moved_child.parent_id = Some(other.id.clone());
    repo.update(&moved_child).unwrap();
    assert_eq!(
        repo.get(&child.id).unwrap().unwrap().parent_id,
        Some(other.id)
    );
}

#[test]
fn folder_delete_cascades_children_and_unfiles_snippets() {
    let conn = fresh_db();
    let folders = FolderRepo::new(&conn);
    let root = folder("Root", None);
    folders.insert(&root).unwrap();
    let child = folder("Child", Some(&root.id));
    folders.insert(&child).unwrap();

    let snippets = SnippetRepo::new(&conn);
    let mut s = snippet("Filed", "body");
    s.folder_id = Some(root.id.clone());
    snippets.insert(&s).unwrap();

    folders.delete(&root.id).unwrap();
    assert!(folders.get(&child.id).unwrap().is_none());
    assert_eq!(snippets.get(&s.id).unwrap().unwrap().folder_id, None);
}

#[test]
fn tag_names_are_unique_and_renameable() {
    let conn = fresh_db();
    let repo = TagRepo::new(&conn);
    let shell = tag("shell");
    repo.insert(&shell).unwrap();

    let err = repo.insert(&tag("shell")).unwrap_err();
    assert!(matches!(err, RepoError::Conflict(_)));

    repo.rename(&shell.id, "terminal").unwrap();
    assert_eq!(repo.get(&shell.id).unwrap().unwrap().name, "terminal");

    let docker = tag("docker");
    repo.insert(&docker).unwrap();
    let err = repo.rename(&docker.id, "terminal").unwrap_err();
    assert!(matches!(err, RepoError::Conflict(_)));
}

#[test]
fn tag_delete_detaches_links_but_keeps_snippets() {
    let conn = fresh_db();
    let tags = TagRepo::new(&conn);
    let snippets = SnippetRepo::new(&conn);
    let t = tag("shell");
    let s = snippet("Kept", "body");
    tags.insert(&t).unwrap();
    snippets.insert(&s).unwrap();
    snippets
        .batch_add_tag(std::slice::from_ref(&s.id), &t.id)
        .unwrap();

    tags.delete(&t.id).unwrap();
    assert!(snippets.tag_ids_of(&s.id).unwrap().is_empty());
    assert!(snippets.get(&s.id).unwrap().is_some());
}

#[test]
fn tag_list_all_is_name_ordered() {
    let conn = fresh_db();
    let repo = TagRepo::new(&conn);
    repo.insert(&tag("zsh")).unwrap();
    repo.insert(&tag("aws")).unwrap();
    let names: Vec<String> = repo
        .list_all()
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(names, ["aws", "zsh"]);
}

#[test]
fn new_id_produces_unique_uuid_text() {
    let a = new_id();
    let b = new_id();
    assert_ne!(a, b);
    assert_eq!(a.len(), 36);
    assert_eq!(a.chars().filter(|c| *c == '-').count(), 4);
}

#[test]
fn list_scoped_filters_each_scope_and_excludes_trash() {
    let conn = fresh_db();
    let folders = FolderRepo::new(&conn);
    let infra = folder("Infra", None);
    folders.insert(&infra).unwrap();

    let repo = SnippetRepo::new(&conn);
    let mut starred = snippet("Starred one", "body");
    starred.is_favorite = true;
    starred.folder_id = Some(infra.id.clone());
    let mut used = snippet("Used one", "body");
    used.last_used_at = Some(9_000);
    used.usage_count = 3;
    let mut used_later = snippet("Used two", "body");
    used_later.last_used_at = Some(12_000);
    used_later.usage_count = 1;
    let mut text_kind = snippet("Text one", "body");
    text_kind.snippet_type = SnippetType::Text;
    let mut trashed = snippet("Trashed", "body");
    trashed.deleted_at = Some(5_000);
    for s in [&starred, &used, &used_later, &text_kind, &trashed] {
        repo.insert(s).unwrap();
    }

    let all = repo.list_scoped(ListScope::All, None, 50, 0).unwrap();
    assert_eq!(all.len(), 4, "trashed rows never appear in a scope");

    let starred_rows = repo.list_scoped(ListScope::Starred, None, 50, 0).unwrap();
    assert_eq!(
        starred_rows.iter().map(|s| &s.title).collect::<Vec<_>>(),
        ["Starred one"]
    );

    // Recent = used at least once, most recently used first.
    let recent = repo.list_scoped(ListScope::Recent, None, 50, 0).unwrap();
    assert_eq!(
        recent.iter().map(|s| &s.title).collect::<Vec<_>>(),
        ["Used two", "Used one"]
    );

    // Used = used at least once, most often used first — the reverse of
    // Recent on this fixture, which is the point: a screen headed "most used"
    // filled from Recent would print these two the wrong way round.
    let most_used = repo.list_scoped(ListScope::Used, None, 50, 0).unwrap();
    assert_eq!(
        most_used.iter().map(|s| &s.title).collect::<Vec<_>>(),
        ["Used one", "Used two"]
    );

    let unsorted = repo.list_scoped(ListScope::Unsorted, None, 50, 0).unwrap();
    assert_eq!(unsorted.len(), 3, "folderless live rows only");

    let in_folder = repo
        .list_scoped(ListScope::Folder(&infra.id), None, 50, 0)
        .unwrap();
    assert_eq!(
        in_folder.iter().map(|s| &s.title).collect::<Vec<_>>(),
        ["Starred one"]
    );

    // The type filter narrows any scope.
    let texts = repo
        .list_scoped(ListScope::All, Some(SnippetType::Text), 50, 0)
        .unwrap();
    assert_eq!(
        texts.iter().map(|s| &s.title).collect::<Vec<_>>(),
        ["Text one"]
    );
}

#[test]
fn list_scoped_ordered_applies_one_order_inside_any_scope() {
    let conn = fresh_db();
    let folders = FolderRepo::new(&conn);
    let mail = folder("Mail", None);
    folders.insert(&mail).unwrap();
    let repo = SnippetRepo::new(&conn);

    let mut old_favourite = snippet("Old favourite", "body");
    old_favourite.created_at = 1_000;
    old_favourite.updated_at = 1_000;
    old_favourite.last_used_at = Some(5_000);
    old_favourite.usage_count = 9;
    let mut just_used = snippet("Just used", "body");
    just_used.created_at = 2_000;
    just_used.updated_at = 2_000;
    just_used.last_used_at = Some(8_000);
    just_used.usage_count = 1;
    let mut never_used = snippet("Never used", "body");
    never_used.created_at = 3_000;
    never_used.updated_at = 3_000;
    for s in [&mut old_favourite, &mut just_used, &mut never_used] {
        s.folder_id = Some(mail.id.clone());
    }
    // Outside the folder, newest and most used of all: it must not leak in.
    let mut elsewhere = snippet("Elsewhere", "body");
    elsewhere.created_at = 4_000;
    elsewhere.updated_at = 4_000;
    elsewhere.last_used_at = Some(9_000);
    elsewhere.usage_count = 20;
    for s in [&old_favourite, &just_used, &never_used, &elsewhere] {
        repo.insert(s).unwrap();
    }

    let titles = |order: ListOrder| -> Vec<String> {
        repo.list_scoped_ordered(ListScope::Folder(&mail.id), None, order, 50, 0)
            .unwrap()
            .into_iter()
            .map(|s| s.title)
            .collect()
    };
    // Never used sorts after every used snippet, not before them.
    assert_eq!(
        titles(ListOrder::LastUsed),
        ["Just used", "Old favourite", "Never used"]
    );
    assert_eq!(
        titles(ListOrder::Created),
        ["Never used", "Just used", "Old favourite"]
    );
    assert_eq!(
        titles(ListOrder::UsageCount),
        ["Old favourite", "Just used", "Never used"]
    );
}

#[test]
fn count_scoped_matches_list_scoped() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let mut fav = snippet("Fav", "body");
    fav.is_favorite = true;
    let mut text_kind = snippet("Plain text", "body");
    text_kind.snippet_type = SnippetType::Text;
    let plain = snippet("Plain command", "body");
    for s in [&fav, &text_kind, &plain] {
        repo.insert(s).unwrap();
    }

    assert_eq!(repo.count_scoped(ListScope::All, None).unwrap(), 3);
    assert_eq!(repo.count_scoped(ListScope::Starred, None).unwrap(), 1);
    assert_eq!(repo.count_scoped(ListScope::Recent, None).unwrap(), 0);
    assert_eq!(
        repo.count_scoped(ListScope::All, Some(SnippetType::Command))
            .unwrap(),
        2
    );

    repo.soft_delete(&plain.id, 10_000).unwrap();
    assert_eq!(repo.count_scoped(ListScope::All, None).unwrap(), 2);
}

#[test]
fn count_by_folder_groups_live_rows_only() {
    let conn = fresh_db();
    let folders = FolderRepo::new(&conn);
    let a = folder("A", None);
    let b = folder("B", None);
    let empty = folder("Empty", None);
    for f in [&a, &b, &empty] {
        folders.insert(f).unwrap();
    }

    let repo = SnippetRepo::new(&conn);
    let mut one = snippet("One", "body");
    one.folder_id = Some(a.id.clone());
    let mut two = snippet("Two", "body");
    two.folder_id = Some(a.id.clone());
    let mut three = snippet("Three", "body");
    three.folder_id = Some(b.id.clone());
    let mut gone = snippet("Gone", "body");
    gone.folder_id = Some(b.id.clone());
    gone.deleted_at = Some(5_000);
    let loose = snippet("Loose", "body");
    for s in [&one, &two, &three, &gone, &loose] {
        repo.insert(s).unwrap();
    }

    let mut counts = repo.count_by_folder().unwrap();
    counts.sort();
    let mut expected = vec![(a.id.clone(), 2), (b.id.clone(), 1)];
    expected.sort();
    assert_eq!(
        counts, expected,
        "trashed and folderless rows are excluded; empty folders absent"
    );
}

#[test]
fn list_triggered_active_returns_only_espanso_eligible_snippets() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);

    let mut eligible = snippet("Eligible", "body");
    eligible.trigger = Some(":sig".to_string());
    eligible.trigger_mode = Some(TriggerMode::Immediate);

    let mut no_trigger = snippet("No trigger", "body");
    no_trigger.trigger = None;

    let mut disabled = snippet("Disabled", "body");
    disabled.trigger = Some(":off".to_string());
    disabled.trigger_mode = Some(TriggerMode::Immediate);
    disabled.is_enabled = false;

    let mut sensitive = snippet("Sensitive", "body");
    sensitive.trigger = Some(":secret".to_string());
    sensitive.trigger_mode = Some(TriggerMode::Immediate);
    sensitive.security_level = SecurityLevel::Sensitive;
    sensitive.content = SnippetContent::Ciphertext(vec![1, 2, 3]);
    sensitive.snippet_type = SnippetType::Sensitive;

    let mut trashed = snippet("Trashed", "body");
    trashed.trigger = Some(":gone".to_string());
    trashed.trigger_mode = Some(TriggerMode::Immediate);
    trashed.deleted_at = Some(9_000);

    for s in [&eligible, &no_trigger, &disabled, &sensitive, &trashed] {
        repo.insert(s).unwrap();
    }

    let got = repo.list_triggered_active().unwrap();
    let ids: Vec<&str> = got.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec![eligible.id.as_str()]);
}

// ---- app rules ---------------------------------------------------------

fn app_rule(snippet_id: &str, rule_type: AppRuleType, app: &str) -> AppRule {
    AppRule {
        id: new_id(),
        snippet_id: snippet_id.to_string(),
        platform: Platform::Macos,
        app_identifier: app.to_string(),
        rule_type,
        window_title_pattern: None,
    }
}

#[test]
fn app_rule_insert_get_update_delete_round_trips() {
    let conn = fresh_db();
    let s = snippet("Ruled", "body");
    SnippetRepo::new(&conn).insert(&s).unwrap();
    let repo = AppRuleRepo::new(&conn);
    let mut rule = app_rule(&s.id, AppRuleType::ShowOnly, "com.apple.Terminal");

    repo.insert(&rule).unwrap();
    assert_eq!(repo.get(&rule.id).unwrap().unwrap(), rule);

    rule.rule_type = AppRuleType::Disable;
    rule.app_identifier = "com.apple.Safari".to_string();
    repo.update(&rule).unwrap();
    assert_eq!(repo.get(&rule.id).unwrap().unwrap(), rule);

    repo.delete(&rule.id).unwrap();
    assert_eq!(repo.get(&rule.id).unwrap(), None);
    assert!(matches!(repo.delete(&rule.id), Err(RepoError::NotFound)));
}

#[test]
fn app_rule_insert_rejects_missing_snippet_and_blank_identifier() {
    let conn = fresh_db();
    let repo = AppRuleRepo::new(&conn);
    let orphan = app_rule("missing-snippet", AppRuleType::Disable, "com.apple.Safari");
    assert!(matches!(repo.insert(&orphan), Err(RepoError::Conflict(_))));

    let s = snippet("Ruled", "body");
    SnippetRepo::new(&conn).insert(&s).unwrap();
    let blank = app_rule(&s.id, AppRuleType::Disable, "   ");
    assert!(matches!(repo.insert(&blank), Err(RepoError::Validation(_))));
}

#[test]
fn app_rule_lists_by_snippet_and_platform_and_cascades_with_the_snippet() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let a = snippet("A", "body");
    let b = snippet("B", "body");
    snippets.insert(&a).unwrap();
    snippets.insert(&b).unwrap();
    let repo = AppRuleRepo::new(&conn);
    let rule_a = app_rule(&a.id, AppRuleType::ShowOnly, "com.apple.Terminal");
    let mut rule_b = app_rule(&b.id, AppRuleType::Disable, "com.apple.Safari");
    rule_b.platform = Platform::Windows;
    repo.insert(&rule_a).unwrap();
    repo.insert(&rule_b).unwrap();

    assert_eq!(repo.list_for_snippet(&a.id).unwrap(), vec![rule_a.clone()]);
    // Platform listing pages and filters.
    assert_eq!(
        repo.list_for_platform(Platform::Macos, 10, 0).unwrap(),
        vec![rule_a]
    );
    assert_eq!(
        repo.list_for_platform(Platform::Macos, 10, 1).unwrap(),
        vec![]
    );

    // Deleting the snippet cascades its rules away (FK ON DELETE CASCADE).
    snippets.delete(&a.id).unwrap();
    assert_eq!(repo.list_for_snippet(&a.id).unwrap(), vec![]);
}

//! Three-way entity-document merge.
//!
//! Inputs are payload documents: the shadow base (last state agreed
//! with the server), the local head (current local state), and the remote
//! head (the verified incoming record). Rules, in order: a field changed on
//! one side only takes that side (rule 1); a field changed on both sides
//! resolves by last-writer-wins on the record clock with the device id as
//! a deterministic tie-break (rule 2); the snippet body block resolves to
//! the remote side and reports a conflict so the caller can preserve the
//! local version as a conflict copy (rule 3); usage fields take the
//! maximum (rule 4); structural entities go through rules 1–2 field by
//! field (rule 5). The merge is symmetric: swapping which side is "local"
//! yields the same merged document apart from rule 3's side assignment.
//!
//! The body block is the four columns that the snippet model binds
//! together (snippet_type, security_level, and the content column pair):
//! resolving them independently could produce an invalid entity (a
//! sensitive level over a plaintext body), so they move as one unit. For
//! sensitive bodies the comparison is over the base64 of the stored
//! K_vault envelope — this module never sees vault plaintext.

use serde_json::{Map, Value};
use typvia_core::model::SyncEntityType;
use zeroize::Zeroizing;

use crate::payload::PayloadError;
use crate::record::SUPPORTED_PAYLOAD_VERSION;

/// The snippet columns that move as one indivisible body block (rule 3).
const BODY_FIELDS: [&str; 4] = [
    "content_plaintext",
    "content_ciphertext",
    "snippet_type",
    "security_level",
];
/// Trigger and its mode are set together (model invariant): they resolve
/// as one unit under rule 2.
const TRIGGER_FIELDS: [&str; 2] = ["trigger", "trigger_mode"];
/// Usage fields take the maximum (rule 4); they never conflict.
const USAGE_MAX_FIELDS: [&str; 2] = ["usage_count", "last_used_at"];

/// One side of the merge: the wrapped document plus the record-level clock
/// and producer used for rule 2 (uniform across entity types — structural
/// documents carry no `updated_at` field of their own).
pub struct MergeSide<'a> {
    pub document: &'a [u8],
    pub updated_at: i64,
    pub device_id: &'a str,
}

/// What the merge produced. Document buffers are zeroized on drop; the
/// transient JSON values live only for the caller's savepoint scope.
pub struct MergeOutcome {
    /// The merged wrapped document, ready to apply and re-queue.
    pub document: Zeroizing<Vec<u8>>,
    /// True when rule 3 fired: the remote body went into `document` and the
    /// local body must be preserved as a conflict copy.
    pub body_conflict: bool,
    /// The local side's entity object (conflict-copy source).
    pub local_entity: Value,
}

/// Merges one entity document three ways. `base` is `None` when the entity
/// was never content-synced (both sides created it independently): every
/// differing field then counts as changed on both sides.
pub fn merge_documents(
    entity_type: SyncEntityType,
    base: Option<&[u8]>,
    local: &MergeSide<'_>,
    remote: &MergeSide<'_>,
) -> Result<MergeOutcome, PayloadError> {
    let base_entity = match base {
        Some(bytes) => entity_object(bytes)?,
        None => Map::new(),
    };
    let local_entity = entity_object(local.document)?;
    let remote_entity = entity_object(remote.document)?;
    let local_wins = lww_prefers_local(local, remote);

    let mut merged = Map::new();
    let mut body_conflict = false;

    // Group fields first (snippet only), then everything else field by
    // field. Iteration order does not matter: serde_json's map sorts keys,
    // so the serialized result is deterministic.
    if entity_type == SyncEntityType::Snippet {
        let base_body = projection(&base_entity, &BODY_FIELDS);
        let local_body = projection(&local_entity, &BODY_FIELDS);
        let remote_body = projection(&remote_entity, &BODY_FIELDS);
        let body_source = if local_body == remote_body || local_body == base_body {
            &remote_entity
        } else if remote_body == base_body {
            &local_entity
        } else {
            // Rule 3: both sides changed the body — the remote version
            // enters the entity, the caller preserves the local version.
            body_conflict = true;
            &remote_entity
        };
        for field in BODY_FIELDS {
            merged.insert(field.to_string(), lookup(body_source, field));
        }

        let trigger_source = pick_unit(
            &projection(&base_entity, &TRIGGER_FIELDS),
            &projection(&local_entity, &TRIGGER_FIELDS),
            &projection(&remote_entity, &TRIGGER_FIELDS),
            local_wins,
        );
        let trigger_side = if trigger_source {
            &local_entity
        } else {
            &remote_entity
        };
        for field in TRIGGER_FIELDS {
            merged.insert(field.to_string(), lookup(trigger_side, field));
        }

        for field in USAGE_MAX_FIELDS {
            merged.insert(
                field.to_string(),
                max_value(lookup(&local_entity, field), lookup(&remote_entity, field)),
            );
        }

        // The merged state is new on both sides: its entity version moves
        // past both inputs (deterministic: max is symmetric).
        let version = local_entity
            .get("version")
            .and_then(Value::as_u64)
            .max(remote_entity.get("version").and_then(Value::as_u64))
            .ok_or(PayloadError::Malformed)?;
        merged.insert(
            "version".to_string(),
            Value::from(version.saturating_add(1)),
        );
    }

    for field in local_entity.keys().chain(remote_entity.keys()) {
        if merged.contains_key(field) {
            continue;
        }
        let base_value = lookup(&base_entity, field);
        let local_value = lookup(&local_entity, field);
        let remote_value = lookup(&remote_entity, field);
        // Rule 1: a single-sided change takes the changed side; rule 2:
        // a double-sided change goes to the LWW winner.
        let take_local = local_value != remote_value
            && local_value != base_value
            && (remote_value == base_value || local_wins);
        let value = if take_local {
            local_value
        } else {
            remote_value
        };
        merged.insert(field.clone(), value);
    }

    Ok(MergeOutcome {
        document: wrap_entity(Value::Object(merged))?,
        body_conflict,
        local_entity: Value::Object(local_entity),
    })
}

/// Builds the conflict-copy entity for merge rule 3: the local body under
/// a fresh identity, suffixed title, same folder as the merged entity, no
/// trigger (it would collide with the source's), fresh usage, and the
/// `conflict_of` marker pointing at the source. Returns the wrapped
/// document ready to apply as a new snippet.
pub fn conflict_copy_document(
    local_entity: &Value,
    merged_folder_id: Value,
    copy_id: &str,
    device_name: &str,
    now: i64,
) -> Result<Zeroizing<Vec<u8>>, PayloadError> {
    let mut copy = local_entity
        .as_object()
        .cloned()
        .ok_or(PayloadError::Malformed)?;
    let source_id = copy
        .get("id")
        .and_then(Value::as_str)
        .ok_or(PayloadError::Malformed)?
        .to_string();
    let title = copy
        .get("title")
        .and_then(Value::as_str)
        .ok_or(PayloadError::Malformed)?;
    copy.insert(
        "title".to_string(),
        Value::from(format!("{title} (conflict on {device_name})")),
    );
    copy.insert("id".to_string(), Value::from(copy_id));
    copy.insert("conflict_of".to_string(), Value::from(source_id));
    copy.insert("folder_id".to_string(), merged_folder_id);
    copy.insert("trigger".to_string(), Value::Null);
    copy.insert("trigger_mode".to_string(), Value::Null);
    copy.insert("usage_count".to_string(), Value::from(0));
    copy.insert("last_used_at".to_string(), Value::Null);
    copy.insert("version".to_string(), Value::from(1));
    copy.insert("created_at".to_string(), Value::from(now));
    copy.insert("updated_at".to_string(), Value::from(now));
    copy.insert("deleted_at".to_string(), Value::Null);
    wrap_entity(Value::Object(copy))
}

/// Rule 2 winner: the later record clock, ties broken by the
/// lexicographically larger device id — both devices compute the same
/// winner independently.
fn lww_prefers_local(local: &MergeSide<'_>, remote: &MergeSide<'_>) -> bool {
    (local.updated_at, local.device_id) > (remote.updated_at, remote.device_id)
}

/// Rule 1/2 over a multi-field unit: true selects the local side.
fn pick_unit(base: &[Value], local: &[Value], remote: &[Value], local_wins: bool) -> bool {
    if local == remote || local == base {
        false
    } else if remote == base {
        true
    } else {
        local_wins
    }
}

fn projection(entity: &Map<String, Value>, fields: &[&str]) -> Vec<Value> {
    fields.iter().map(|f| lookup(entity, f)).collect()
}

fn lookup(entity: &Map<String, Value>, field: &str) -> Value {
    entity.get(field).cloned().unwrap_or(Value::Null)
}

/// Maximum of two JSON numbers where null orders below everything
/// (rule 4: usage_count and last_used_at).
fn max_value(a: Value, b: Value) -> Value {
    match (a.as_i64(), b.as_i64()) {
        (Some(x), Some(y)) => Value::from(x.max(y)),
        (Some(_), None) => a,
        (None, Some(_)) => b,
        (None, None) => Value::Null,
    }
}

fn entity_object(document: &[u8]) -> Result<Map<String, Value>, PayloadError> {
    let doc: Value = serde_json::from_slice(document).map_err(|_| PayloadError::Malformed)?;
    if doc.get("payload_version").and_then(Value::as_u64) != Some(SUPPORTED_PAYLOAD_VERSION) {
        return Err(PayloadError::Malformed);
    }
    doc.get("entity")
        .and_then(Value::as_object)
        .cloned()
        .ok_or(PayloadError::Malformed)
}

fn wrap_entity(entity: Value) -> Result<Zeroizing<Vec<u8>>, PayloadError> {
    serde_json::to_vec(&serde_json::json!({
        "payload_version": SUPPORTED_PAYLOAD_VERSION,
        "entity": entity,
    }))
    .map(Zeroizing::new)
    .map_err(|_| PayloadError::Malformed)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn snippet_doc(title: &str, body: &str, folder: Option<&str>, updated_at: i64) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "payload_version": 1,
            "entity": {
                "id": "s1",
                "workspace_id": "default",
                "title": title,
                "content_plaintext": body,
                "content_ciphertext": null,
                "snippet_type": "text",
                "description": null,
                "folder_id": folder,
                "trigger": null,
                "trigger_mode": null,
                "language": null,
                "security_level": "normal",
                "is_favorite": false,
                "is_pinned": false,
                "is_enabled": true,
                "platform_scope": [],
                "created_at": 1000,
                "updated_at": updated_at,
                "last_used_at": null,
                "usage_count": 0,
                "version": 1,
                "deleted_at": null,
                "conflict_of": null,
            }
        }))
        .unwrap()
    }

    fn side<'a>(document: &'a [u8], updated_at: i64, device_id: &'a str) -> MergeSide<'a> {
        MergeSide {
            document,
            updated_at,
            device_id,
        }
    }

    fn entity(outcome: &MergeOutcome) -> Value {
        let doc: Value = serde_json::from_slice(&outcome.document).unwrap();
        doc.get("entity").unwrap().clone()
    }

    #[test]
    fn single_sided_changes_merge_field_for_field() {
        let base = snippet_doc("Base", "body", None, 10);
        let local = snippet_doc("Renamed locally", "body", None, 20);
        let remote = snippet_doc("Base", "body", Some("f1"), 30);
        let outcome = merge_documents(
            SyncEntityType::Snippet,
            Some(&base),
            &side(&local, 20, "dev-a"),
            &side(&remote, 30, "dev-b"),
        )
        .unwrap();
        let merged = entity(&outcome);
        assert_eq!(merged["title"], "Renamed locally");
        assert_eq!(merged["folder_id"], "f1");
        assert_eq!(merged["content_plaintext"], "body");
        assert!(!outcome.body_conflict);
    }

    #[test]
    fn a_double_edited_scalar_resolves_by_the_later_clock() {
        let base = snippet_doc("Base", "body", None, 10);
        let local = snippet_doc("Local title", "body", None, 50);
        let remote = snippet_doc("Remote title", "body", None, 20);
        let outcome = merge_documents(
            SyncEntityType::Snippet,
            Some(&base),
            &side(&local, 50, "dev-a"),
            &side(&remote, 20, "dev-b"),
        )
        .unwrap();
        assert_eq!(entity(&outcome)["title"], "Local title");
    }

    #[test]
    fn the_device_id_tiebreak_gives_the_same_winner_from_both_ends() {
        let base = snippet_doc("Base", "body", None, 10);
        let alpha = snippet_doc("Alpha title", "body", None, 20);
        let beta = snippet_doc("Beta title", "body", None, 20);

        // Merged on the alpha device (alpha is local)...
        let on_alpha = merge_documents(
            SyncEntityType::Snippet,
            Some(&base),
            &side(&alpha, 20, "dev-a"),
            &side(&beta, 20, "dev-b"),
        )
        .unwrap();
        // ...and on the beta device (roles swapped).
        let on_beta = merge_documents(
            SyncEntityType::Snippet,
            Some(&base),
            &side(&beta, 20, "dev-b"),
            &side(&alpha, 20, "dev-a"),
        )
        .unwrap();
        assert_eq!(entity(&on_alpha)["title"], "Beta title");
        assert_eq!(entity(&on_alpha), entity(&on_beta));
    }

    #[test]
    fn concurrent_body_edits_keep_the_remote_body_and_flag_the_conflict() {
        let base = snippet_doc("Base", "base body", None, 10);
        let local = snippet_doc("Base", "local body", None, 20);
        let remote = snippet_doc("Base", "remote body", None, 30);
        let outcome = merge_documents(
            SyncEntityType::Snippet,
            Some(&base),
            &side(&local, 20, "dev-a"),
            &side(&remote, 30, "dev-b"),
        )
        .unwrap();
        assert!(outcome.body_conflict);
        assert_eq!(entity(&outcome)["content_plaintext"], "remote body");
        assert_eq!(outcome.local_entity["content_plaintext"], "local body");
    }

    #[test]
    fn identical_body_edits_do_not_conflict() {
        let base = snippet_doc("Base", "base body", None, 10);
        let local = snippet_doc("Base", "same edit", None, 20);
        let remote = snippet_doc("Base", "same edit", None, 30);
        let outcome = merge_documents(
            SyncEntityType::Snippet,
            Some(&base),
            &side(&local, 20, "dev-a"),
            &side(&remote, 30, "dev-b"),
        )
        .unwrap();
        assert!(!outcome.body_conflict);
        assert_eq!(entity(&outcome)["content_plaintext"], "same edit");
    }

    #[test]
    fn a_one_sided_sensitive_conversion_moves_as_one_block() {
        let base = snippet_doc("Base", "plain body", None, 10);
        // Local converted to sensitive: type, level, and body flip together.
        let local = serde_json::to_vec(&serde_json::json!({
            "payload_version": 1,
            "entity": {
                "id": "s1", "workspace_id": "default", "title": "Base",
                "content_plaintext": null, "content_ciphertext": "AQIDBA==",
                "snippet_type": "sensitive", "description": null, "folder_id": null,
                "trigger": null, "trigger_mode": null, "language": null,
                "security_level": "sensitive", "is_favorite": false, "is_pinned": false,
                "is_enabled": true, "platform_scope": [], "created_at": 1000,
                "updated_at": 20, "last_used_at": null, "usage_count": 0,
                "version": 2, "deleted_at": null, "conflict_of": null,
            }
        }))
        .unwrap();
        let remote = snippet_doc("Remote rename", "plain body", None, 30);
        let outcome = merge_documents(
            SyncEntityType::Snippet,
            Some(&base),
            &side(&local, 20, "dev-a"),
            &side(&remote, 30, "dev-b"),
        )
        .unwrap();
        let merged = entity(&outcome);
        assert!(!outcome.body_conflict);
        assert_eq!(merged["security_level"], "sensitive");
        assert_eq!(merged["snippet_type"], "sensitive");
        assert_eq!(merged["content_ciphertext"], "AQIDBA==");
        assert_eq!(merged["content_plaintext"], Value::Null);
        assert_eq!(merged["title"], "Remote rename");
    }

    #[test]
    fn usage_fields_take_the_maximum() {
        let base = snippet_doc("Base", "body", None, 10);
        let mut local: Value = serde_json::from_slice(&base).unwrap();
        local["entity"]["usage_count"] = Value::from(7);
        local["entity"]["last_used_at"] = Value::from(500);
        let mut remote: Value = serde_json::from_slice(&base).unwrap();
        remote["entity"]["usage_count"] = Value::from(3);
        remote["entity"]["last_used_at"] = Value::Null;
        let local_bytes = serde_json::to_vec(&local).unwrap();
        let remote_bytes = serde_json::to_vec(&remote).unwrap();
        let outcome = merge_documents(
            SyncEntityType::Snippet,
            Some(&base),
            &side(&local_bytes, 20, "dev-a"),
            &side(&remote_bytes, 30, "dev-b"),
        )
        .unwrap();
        let merged = entity(&outcome);
        assert_eq!(merged["usage_count"], 7);
        assert_eq!(merged["last_used_at"], 500);
    }

    #[test]
    fn the_merged_snippet_version_moves_past_both_inputs() {
        let base = snippet_doc("Base", "body", None, 10);
        let mut local: Value = serde_json::from_slice(&base).unwrap();
        local["entity"]["version"] = Value::from(4);
        local["entity"]["title"] = Value::from("Local");
        let remote = snippet_doc("Base", "body", Some("f1"), 30);
        let local_bytes = serde_json::to_vec(&local).unwrap();
        let outcome = merge_documents(
            SyncEntityType::Snippet,
            Some(&base),
            &side(&local_bytes, 20, "dev-a"),
            &side(&remote, 30, "dev-b"),
        )
        .unwrap();
        assert_eq!(entity(&outcome)["version"], 5);
    }

    #[test]
    fn a_structural_entity_merges_by_rules_one_and_two() {
        let folder = |name: &str, sort: i32| {
            serde_json::to_vec(&serde_json::json!({
                "payload_version": 1,
                "entity": {
                    "id": "f1", "parent_id": null, "name": name,
                    "sort_order": sort, "created_at": 1, "updated_at": 2,
                }
            }))
            .unwrap()
        };
        let base = folder("Work", 0);
        let local = folder("Projects", 0);
        let remote = folder("Work", 9);
        let outcome = merge_documents(
            SyncEntityType::Folder,
            Some(&base),
            &side(&local, 20, "dev-a"),
            &side(&remote, 30, "dev-b"),
        )
        .unwrap();
        let merged = entity(&outcome);
        assert_eq!(merged["name"], "Projects");
        assert_eq!(merged["sort_order"], 9);
        assert!(!outcome.body_conflict);
    }

    #[test]
    fn a_missing_base_treats_differing_fields_as_double_edits() {
        let local = snippet_doc("Local", "local body", None, 20);
        let remote = snippet_doc("Remote", "remote body", None, 30);
        let outcome = merge_documents(
            SyncEntityType::Snippet,
            None,
            &side(&local, 20, "dev-a"),
            &side(&remote, 30, "dev-b"),
        )
        .unwrap();
        assert!(outcome.body_conflict);
        let merged = entity(&outcome);
        assert_eq!(merged["title"], "Remote");
        assert_eq!(merged["content_plaintext"], "remote body");
    }

    #[test]
    fn the_conflict_copy_carries_the_marker_and_drops_the_trigger() {
        let mut local: Value =
            serde_json::from_slice(&snippet_doc("Notes", "local body", None, 20)).unwrap();
        local["entity"]["trigger"] = Value::from(":n");
        local["entity"]["trigger_mode"] = Value::from("immediate");
        local["entity"]["usage_count"] = Value::from(9);
        let copy_bytes = conflict_copy_document(
            &local["entity"],
            Value::from("f-merged"),
            "copy-id",
            "MacBook Pro",
            5_000,
        )
        .unwrap();
        let copy: Value = serde_json::from_slice(&copy_bytes).unwrap();
        let copy = copy.get("entity").unwrap();
        assert_eq!(copy["id"], "copy-id");
        assert_eq!(copy["title"], "Notes (conflict on MacBook Pro)");
        assert_eq!(copy["conflict_of"], "s1");
        assert_eq!(copy["folder_id"], "f-merged");
        assert_eq!(copy["trigger"], Value::Null);
        assert_eq!(copy["trigger_mode"], Value::Null);
        assert_eq!(copy["usage_count"], 0);
        assert_eq!(copy["version"], 1);
        assert_eq!(copy["content_plaintext"], "local body");
    }
}

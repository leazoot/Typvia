//! Entity payload documents.
//!
//! A document is the canonical JSON `{"payload_version": 1, "entity": {…}}`
//! wrapper around one entity's full state, with serde field names matching
//! the database columns. Building reads through the core repositories;
//! applying upserts through them — never through the host write use cases,
//! so applying a pulled record can never re-enter the outbox (echo
//! suppression is structural). Sensitive snippet bodies pass through as
//! their stored K_vault envelope bytes (double envelope): this module
//! never sees vault plaintext.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use typvia_core::model::{
    AppRule, Folder, Snippet, SnippetContent, SyncEntityType, Tag, TemplateField, TimestampMs,
    UnknownEnumValue, ValidationError,
};
use typvia_core::repo::{
    AppRuleRepo, FolderRepo, RepoError, SnippetRepo, TagRepo, TemplateFieldRepo,
};

use crate::record::SUPPORTED_PAYLOAD_VERSION;

/// Separator between the two ids of a `snippet_tag` sync identity
/// (UUIDs never contain it, so the join is unambiguous).
pub const SNIPPET_TAG_ID_SEPARATOR: char = '/';

/// Rejection reasons for building and applying entity documents. Variants
/// carry only structural facts, never entity content.
#[derive(Debug)]
pub enum PayloadError {
    /// The document is not the JSON shape this build supports.
    Malformed,
    /// The document's inner entity id disagrees with the record identity
    /// the envelope AAD was bound to.
    EntityIdMismatch,
    /// This build has no local apply/build path for the entity type
    /// (`ai_action`, until the AI repository exists).
    UnsupportedEntity,
    /// A TEXT enum value in the document is unknown to this build.
    UnknownEnum(UnknownEnumValue),
    /// The reconstructed entity failed model validation.
    Validation(ValidationError),
    /// The repository rejected the read or write.
    Repo(RepoError),
}

impl std::fmt::Display for PayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => f.write_str("entity document is malformed"),
            Self::EntityIdMismatch => f.write_str("document entity id disagrees with the record"),
            Self::UnsupportedEntity => f.write_str("entity type has no local apply path"),
            Self::UnknownEnum(e) => write!(f, "{e}"),
            Self::Validation(e) => write!(f, "{e}"),
            Self::Repo(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PayloadError {}

impl From<RepoError> for PayloadError {
    fn from(e: RepoError) -> Self {
        Self::Repo(e)
    }
}

impl From<UnknownEnumValue> for PayloadError {
    fn from(e: UnknownEnumValue) -> Self {
        Self::UnknownEnum(e)
    }
}

impl From<ValidationError> for PayloadError {
    fn from(e: ValidationError) -> Self {
        Self::Validation(e)
    }
}

#[derive(Serialize, Deserialize)]
struct Document<T> {
    payload_version: u64,
    entity: T,
}

fn wrap<T: Serialize>(entity: T) -> Result<Vec<u8>, PayloadError> {
    serde_json::to_vec(&Document {
        payload_version: SUPPORTED_PAYLOAD_VERSION,
        entity,
    })
    .map_err(|_| PayloadError::Malformed)
}

fn unwrap_doc<T: for<'de> Deserialize<'de>>(document: &[u8]) -> Result<T, PayloadError> {
    let doc: Document<T> = serde_json::from_slice(document).map_err(|_| PayloadError::Malformed)?;
    if doc.payload_version != SUPPORTED_PAYLOAD_VERSION {
        return Err(PayloadError::Malformed);
    }
    Ok(doc.entity)
}

// ----- entity document shapes (field names = database columns) -----

#[derive(Serialize, Deserialize)]
struct SnippetDoc {
    id: String,
    workspace_id: String,
    title: String,
    content_plaintext: Option<String>,
    /// Base64 of the stored K_vault envelope bytes (double envelope).
    content_ciphertext: Option<String>,
    snippet_type: String,
    description: Option<String>,
    folder_id: Option<String>,
    trigger: Option<String>,
    trigger_mode: Option<String>,
    language: Option<String>,
    security_level: String,
    is_favorite: bool,
    is_pinned: bool,
    is_enabled: bool,
    platform_scope: Vec<String>,
    created_at: TimestampMs,
    updated_at: TimestampMs,
    last_used_at: Option<TimestampMs>,
    usage_count: u64,
    version: u32,
    deleted_at: Option<TimestampMs>,
    /// Conflict-copy marker set by merge rule 3; defaulted so
    /// documents sealed before the field existed still apply (same
    /// payload_version, additive field).
    #[serde(default)]
    conflict_of: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct FolderDoc {
    id: String,
    parent_id: Option<String>,
    name: String,
    sort_order: i32,
    created_at: TimestampMs,
    updated_at: TimestampMs,
}

#[derive(Serialize, Deserialize)]
struct TagDoc {
    id: String,
    name: String,
    created_at: TimestampMs,
}

#[derive(Serialize, Deserialize)]
struct SnippetTagDoc {
    snippet_id: String,
    tag_id: String,
}

#[derive(Serialize, Deserialize)]
struct TemplateFieldDoc {
    id: String,
    snippet_id: String,
    name: String,
    label: String,
    field_type: String,
    default_value: Option<String>,
    options: Vec<String>,
    validation: Option<String>,
    is_required: bool,
    sort_order: i32,
    platform_overrides: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct AppRuleDoc {
    id: String,
    snippet_id: String,
    platform: String,
    app_identifier: String,
    rule_type: String,
    window_title_pattern: Option<String>,
}

// ----- model <-> document conversions -----

fn snippet_to_doc(snippet: &Snippet) -> SnippetDoc {
    let (content_plaintext, content_ciphertext) = match &snippet.content {
        SnippetContent::Plaintext(text) => (Some(text.clone()), None),
        SnippetContent::Ciphertext(bytes) => (None, Some(BASE64.encode(bytes))),
    };
    SnippetDoc {
        id: snippet.id.clone(),
        workspace_id: snippet.workspace_id.clone(),
        title: snippet.title.clone(),
        content_plaintext,
        content_ciphertext,
        snippet_type: snippet.snippet_type.as_str().to_string(),
        description: snippet.description.clone(),
        folder_id: snippet.folder_id.clone(),
        trigger: snippet.trigger.clone(),
        trigger_mode: snippet.trigger_mode.map(|m| m.as_str().to_string()),
        language: snippet.language.clone(),
        security_level: snippet.security_level.as_str().to_string(),
        is_favorite: snippet.is_favorite,
        is_pinned: snippet.is_pinned,
        is_enabled: snippet.is_enabled,
        platform_scope: snippet
            .platform_scope
            .iter()
            .map(|p| p.as_str().to_string())
            .collect(),
        created_at: snippet.created_at,
        updated_at: snippet.updated_at,
        last_used_at: snippet.last_used_at,
        usage_count: snippet.usage_count,
        version: snippet.version,
        deleted_at: snippet.deleted_at,
        conflict_of: snippet.conflict_of.clone(),
    }
}

fn doc_to_snippet(doc: SnippetDoc) -> Result<Snippet, PayloadError> {
    let content = match (doc.content_plaintext, doc.content_ciphertext) {
        (Some(text), None) => SnippetContent::Plaintext(text),
        (None, Some(b64)) => {
            SnippetContent::Ciphertext(BASE64.decode(b64).map_err(|_| PayloadError::Malformed)?)
        }
        _ => return Err(PayloadError::Malformed),
    };
    let mut platform_scope = Vec::with_capacity(doc.platform_scope.len());
    for p in doc.platform_scope {
        platform_scope.push(p.parse()?);
    }
    Ok(Snippet {
        id: doc.id,
        workspace_id: doc.workspace_id,
        title: doc.title,
        content,
        snippet_type: doc.snippet_type.parse()?,
        description: doc.description,
        folder_id: doc.folder_id,
        trigger: doc.trigger,
        trigger_mode: doc.trigger_mode.as_deref().map(str::parse).transpose()?,
        language: doc.language,
        security_level: doc.security_level.parse()?,
        is_favorite: doc.is_favorite,
        is_pinned: doc.is_pinned,
        is_enabled: doc.is_enabled,
        platform_scope,
        created_at: doc.created_at,
        updated_at: doc.updated_at,
        last_used_at: doc.last_used_at,
        usage_count: doc.usage_count,
        version: doc.version,
        deleted_at: doc.deleted_at,
        conflict_of: doc.conflict_of,
    })
}

/// Splits a `snippet_tag` sync identity back into its two ids.
fn split_snippet_tag_id(entity_id: &str) -> Result<(&str, &str), PayloadError> {
    entity_id
        .split_once(SNIPPET_TAG_ID_SEPARATOR)
        .filter(|(s, t)| !s.is_empty() && !t.is_empty())
        .ok_or(PayloadError::Malformed)
}

/// The sync identity of a snippet-tag link.
pub fn snippet_tag_entity_id(snippet_id: &str, tag_id: &str) -> String {
    format!("{snippet_id}{SNIPPET_TAG_ID_SEPARATOR}{tag_id}")
}

// ----- build (local state -> document) -----

/// Serializes the current local state of an entity as a payload document.
/// `Ok(None)` when the entity does not exist (locally deleted before the
/// change could be sealed — the caller skips the enqueue).
pub fn build_document(
    conn: &Connection,
    entity_type: SyncEntityType,
    entity_id: &str,
) -> Result<Option<Vec<u8>>, PayloadError> {
    match entity_type {
        SyncEntityType::Snippet => match SnippetRepo::new(conn).get(entity_id)? {
            Some(snippet) => Ok(Some(wrap(snippet_to_doc(&snippet))?)),
            None => Ok(None),
        },
        SyncEntityType::Folder => match FolderRepo::new(conn).get(entity_id)? {
            Some(folder) => Ok(Some(wrap(FolderDoc {
                id: folder.id,
                parent_id: folder.parent_id,
                name: folder.name,
                sort_order: folder.sort_order,
                created_at: folder.created_at,
                updated_at: folder.updated_at,
            })?)),
            None => Ok(None),
        },
        SyncEntityType::Tag => match TagRepo::new(conn).get(entity_id)? {
            Some(tag) => Ok(Some(wrap(TagDoc {
                id: tag.id,
                name: tag.name,
                created_at: tag.created_at,
            })?)),
            None => Ok(None),
        },
        SyncEntityType::SnippetTag => {
            let (snippet_id, tag_id) = split_snippet_tag_id(entity_id)?;
            let linked = SnippetRepo::new(conn)
                .tag_ids_of(snippet_id)?
                .iter()
                .any(|t| t == tag_id);
            if !linked {
                return Ok(None);
            }
            Ok(Some(wrap(SnippetTagDoc {
                snippet_id: snippet_id.to_string(),
                tag_id: tag_id.to_string(),
            })?))
        }
        SyncEntityType::TemplateField => match TemplateFieldRepo::new(conn).get(entity_id)? {
            Some(field) => Ok(Some(wrap(TemplateFieldDoc {
                id: field.id,
                snippet_id: field.snippet_id,
                name: field.name,
                label: field.label,
                field_type: field.field_type.as_str().to_string(),
                default_value: field.default_value,
                options: field.options,
                validation: field.validation,
                is_required: field.is_required,
                sort_order: field.sort_order,
                platform_overrides: field.platform_overrides,
            })?)),
            None => Ok(None),
        },
        SyncEntityType::AppRule => match AppRuleRepo::new(conn).get(entity_id)? {
            Some(rule) => Ok(Some(wrap(AppRuleDoc {
                id: rule.id,
                snippet_id: rule.snippet_id,
                platform: rule.platform.as_str().to_string(),
                app_identifier: rule.app_identifier,
                rule_type: rule.rule_type.as_str().to_string(),
                window_title_pattern: rule.window_title_pattern,
            })?)),
            None => Ok(None),
        },
        SyncEntityType::AiAction => Err(PayloadError::UnsupportedEntity),
    }
}

// ----- apply (document -> local state) -----

/// Applies a verified content document to the local database: full-state
/// upsert through the repositories. The caller wraps this in the pull
/// transaction and isolates per-record failures.
pub fn apply_document(
    conn: &Connection,
    entity_type: SyncEntityType,
    entity_id: &str,
    document: &[u8],
) -> Result<(), PayloadError> {
    match entity_type {
        SyncEntityType::Snippet => {
            let snippet = doc_to_snippet(unwrap_doc::<SnippetDoc>(document)?)?;
            if snippet.id != entity_id {
                return Err(PayloadError::EntityIdMismatch);
            }
            let repo = SnippetRepo::new(conn);
            if repo.get(entity_id)?.is_some() {
                repo.update(&snippet)?;
            } else {
                repo.insert(&snippet)?;
            }
            Ok(())
        }
        SyncEntityType::Folder => {
            let doc: FolderDoc = unwrap_doc(document)?;
            if doc.id != entity_id {
                return Err(PayloadError::EntityIdMismatch);
            }
            let folder = Folder {
                id: doc.id,
                parent_id: doc.parent_id,
                name: doc.name,
                sort_order: doc.sort_order,
                created_at: doc.created_at,
                updated_at: doc.updated_at,
            };
            let repo = FolderRepo::new(conn);
            if repo.get(entity_id)?.is_some() {
                repo.update(&folder)?;
            } else {
                repo.insert(&folder)?;
            }
            Ok(())
        }
        SyncEntityType::Tag => {
            let doc: TagDoc = unwrap_doc(document)?;
            if doc.id != entity_id {
                return Err(PayloadError::EntityIdMismatch);
            }
            let repo = TagRepo::new(conn);
            if repo.get(entity_id)?.is_some() {
                repo.rename(entity_id, &doc.name)?;
            } else {
                repo.insert(&Tag {
                    id: doc.id,
                    name: doc.name,
                    created_at: doc.created_at,
                })?;
            }
            Ok(())
        }
        SyncEntityType::SnippetTag => {
            let doc: SnippetTagDoc = unwrap_doc(document)?;
            if snippet_tag_entity_id(&doc.snippet_id, &doc.tag_id) != entity_id {
                return Err(PayloadError::EntityIdMismatch);
            }
            let repo = SnippetRepo::new(conn);
            let already = repo.tag_ids_of(&doc.snippet_id)?.contains(&doc.tag_id);
            if !already {
                repo.add_tag(&doc.snippet_id, &doc.tag_id)?;
            }
            Ok(())
        }
        SyncEntityType::TemplateField => {
            let doc: TemplateFieldDoc = unwrap_doc(document)?;
            if doc.id != entity_id {
                return Err(PayloadError::EntityIdMismatch);
            }
            TemplateFieldRepo::new(conn).upsert(&TemplateField {
                id: doc.id,
                snippet_id: doc.snippet_id,
                name: doc.name,
                label: doc.label,
                field_type: doc.field_type.parse()?,
                default_value: doc.default_value,
                options: doc.options,
                validation: doc.validation,
                is_required: doc.is_required,
                sort_order: doc.sort_order,
                platform_overrides: doc.platform_overrides,
            })?;
            Ok(())
        }
        SyncEntityType::AppRule => {
            let doc: AppRuleDoc = unwrap_doc(document)?;
            if doc.id != entity_id {
                return Err(PayloadError::EntityIdMismatch);
            }
            let rule = AppRule {
                id: doc.id,
                snippet_id: doc.snippet_id,
                platform: doc.platform.parse()?,
                app_identifier: doc.app_identifier,
                rule_type: doc.rule_type.parse()?,
                window_title_pattern: doc.window_title_pattern,
            };
            let repo = AppRuleRepo::new(conn);
            if repo.get(entity_id)?.is_some() {
                repo.update(&rule)?;
            } else {
                repo.insert(&rule)?;
            }
            Ok(())
        }
        SyncEntityType::AiAction => Err(PayloadError::UnsupportedEntity),
    }
}

/// Applies a verified tombstone: local soft delete for snippets (recycle
/// bin), hard removal for structural entities — mirroring exactly what
/// the producing device's cascade did. The
/// goal state already holding (entity absent) is success, not an error.
pub fn apply_tombstone(
    conn: &Connection,
    entity_type: SyncEntityType,
    entity_id: &str,
    deleted_at: TimestampMs,
) -> Result<(), PayloadError> {
    match entity_type {
        SyncEntityType::Snippet => {
            let repo = SnippetRepo::new(conn);
            match repo.get(entity_id)? {
                Some(snippet) if snippet.deleted_at.is_none() => {
                    repo.soft_delete(entity_id, deleted_at)?;
                    Ok(())
                }
                _ => Ok(()),
            }
        }
        SyncEntityType::Folder => match FolderRepo::new(conn).delete(entity_id) {
            Ok(()) | Err(RepoError::NotFound) => Ok(()),
            Err(e) => Err(e.into()),
        },
        SyncEntityType::Tag => match TagRepo::new(conn).delete(entity_id) {
            Ok(()) | Err(RepoError::NotFound) => Ok(()),
            Err(e) => Err(e.into()),
        },
        SyncEntityType::SnippetTag => {
            let (snippet_id, tag_id) = split_snippet_tag_id(entity_id)?;
            SnippetRepo::new(conn).batch_remove_tag(&[snippet_id.to_string()], tag_id)?;
            Ok(())
        }
        SyncEntityType::TemplateField => {
            TemplateFieldRepo::new(conn).delete(entity_id)?;
            Ok(())
        }
        SyncEntityType::AppRule => match AppRuleRepo::new(conn).delete(entity_id) {
            Ok(()) | Err(RepoError::NotFound) => Ok(()),
            Err(e) => Err(e.into()),
        },
        SyncEntityType::AiAction => Err(PayloadError::UnsupportedEntity),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use typvia_core::db::{migrate_to_latest, open_in_memory};
    use typvia_core::model::{SecurityLevel, SnippetType};

    fn db() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn snippet(id: &str) -> Snippet {
        Snippet {
            id: id.to_string(),
            workspace_id: "default".to_string(),
            title: "Greeting".to_string(),
            content: SnippetContent::Plaintext("Hello from A".to_string()),
            snippet_type: SnippetType::Text,
            description: Some("demo".to_string()),
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
            security_level: SecurityLevel::Normal,
            is_favorite: true,
            is_pinned: false,
            is_enabled: true,
            platform_scope: Vec::new(),
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_001_000,
            last_used_at: None,
            usage_count: 3,
            version: 2,
            deleted_at: None,
            conflict_of: None,
        }
    }

    #[test]
    fn a_snippet_document_round_trips_field_for_field() {
        let source = db();
        let original = snippet("s1");
        SnippetRepo::new(&source).insert(&original).unwrap();
        let document = build_document(&source, SyncEntityType::Snippet, "s1")
            .unwrap()
            .unwrap();

        let target = db();
        apply_document(&target, SyncEntityType::Snippet, "s1", &document).unwrap();
        let restored = SnippetRepo::new(&target).get("s1").unwrap().unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn a_sensitive_snippet_travels_as_its_stored_ciphertext_bytes() {
        let source = db();
        let mut sensitive = snippet("s2");
        sensitive.snippet_type = SnippetType::Sensitive;
        sensitive.security_level = SecurityLevel::Sensitive;
        sensitive.content = SnippetContent::Ciphertext(vec![0x01, 0x02, 0xFE, 0xFF]);
        SnippetRepo::new(&source).insert(&sensitive).unwrap();
        let document = build_document(&source, SyncEntityType::Snippet, "s2")
            .unwrap()
            .unwrap();

        let target = db();
        apply_document(&target, SyncEntityType::Snippet, "s2", &document).unwrap();
        let restored = SnippetRepo::new(&target).get("s2").unwrap().unwrap();
        assert_eq!(
            restored.content,
            SnippetContent::Ciphertext(vec![0x01, 0x02, 0xFE, 0xFF])
        );
    }

    #[test]
    fn applying_over_an_existing_row_replaces_it() {
        let conn = db();
        SnippetRepo::new(&conn).insert(&snippet("s1")).unwrap();
        let mut newer = snippet("s1");
        newer.title = "Renamed remotely".to_string();
        newer.version = 3;
        let document = wrap(snippet_to_doc(&newer)).unwrap();
        apply_document(&conn, SyncEntityType::Snippet, "s1", &document).unwrap();
        assert_eq!(
            SnippetRepo::new(&conn).get("s1").unwrap().unwrap().title,
            "Renamed remotely"
        );
    }

    #[test]
    fn a_mismatched_inner_id_is_rejected() {
        let conn = db();
        let document = wrap(snippet_to_doc(&snippet("s1"))).unwrap();
        assert!(matches!(
            apply_document(&conn, SyncEntityType::Snippet, "other", &document),
            Err(PayloadError::EntityIdMismatch)
        ));
    }

    #[test]
    fn a_document_with_both_content_forms_is_malformed() {
        let conn = db();
        let raw = br#"{"payload_version":1,"entity":{"id":"s1","workspace_id":"d","title":"t",
            "content_plaintext":"x","content_ciphertext":"AA==","snippet_type":"text",
            "description":null,"folder_id":null,"trigger":null,"trigger_mode":null,
            "language":null,"security_level":"normal","is_favorite":false,"is_pinned":false,
            "is_enabled":true,"platform_scope":[],"created_at":1,"updated_at":1,
            "last_used_at":null,"usage_count":0,"version":1,"deleted_at":null}}"#;
        assert!(matches!(
            apply_document(&conn, SyncEntityType::Snippet, "s1", raw),
            Err(PayloadError::Malformed)
        ));
    }

    #[test]
    fn folder_and_tag_documents_round_trip() {
        let source = db();
        FolderRepo::new(&source)
            .insert(&Folder {
                id: "f1".to_string(),
                parent_id: None,
                name: "Work".to_string(),
                sort_order: 1,
                created_at: 1,
                updated_at: 2,
            })
            .unwrap();
        TagRepo::new(&source)
            .insert(&Tag {
                id: "t1".to_string(),
                name: "urgent".to_string(),
                created_at: 3,
            })
            .unwrap();

        let target = db();
        let folder_doc = build_document(&source, SyncEntityType::Folder, "f1")
            .unwrap()
            .unwrap();
        let tag_doc = build_document(&source, SyncEntityType::Tag, "t1")
            .unwrap()
            .unwrap();
        apply_document(&target, SyncEntityType::Folder, "f1", &folder_doc).unwrap();
        apply_document(&target, SyncEntityType::Tag, "t1", &tag_doc).unwrap();
        assert_eq!(
            FolderRepo::new(&target).get("f1").unwrap().unwrap().name,
            "Work"
        );
        assert_eq!(
            TagRepo::new(&target).get("t1").unwrap().unwrap().name,
            "urgent"
        );
    }

    #[test]
    fn snippet_tag_link_round_trips_and_tombstones() {
        let source = db();
        SnippetRepo::new(&source).insert(&snippet("s1")).unwrap();
        TagRepo::new(&source)
            .insert(&Tag {
                id: "t1".to_string(),
                name: "urgent".to_string(),
                created_at: 3,
            })
            .unwrap();
        SnippetRepo::new(&source).add_tag("s1", "t1").unwrap();

        let entity_id = snippet_tag_entity_id("s1", "t1");
        let document = build_document(&source, SyncEntityType::SnippetTag, &entity_id)
            .unwrap()
            .unwrap();

        // Apply onto a target that already has both endpoints.
        let target = db();
        SnippetRepo::new(&target).insert(&snippet("s1")).unwrap();
        TagRepo::new(&target)
            .insert(&Tag {
                id: "t1".to_string(),
                name: "urgent".to_string(),
                created_at: 3,
            })
            .unwrap();
        apply_document(&target, SyncEntityType::SnippetTag, &entity_id, &document).unwrap();
        // Idempotent re-apply.
        apply_document(&target, SyncEntityType::SnippetTag, &entity_id, &document).unwrap();
        assert_eq!(
            SnippetRepo::new(&target).tag_ids_of("s1").unwrap(),
            vec!["t1".to_string()]
        );

        apply_tombstone(&target, SyncEntityType::SnippetTag, &entity_id, 9).unwrap();
        assert!(
            SnippetRepo::new(&target)
                .tag_ids_of("s1")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_snippet_tombstone_is_a_local_soft_delete() {
        let conn = db();
        SnippetRepo::new(&conn).insert(&snippet("s1")).unwrap();
        apply_tombstone(&conn, SyncEntityType::Snippet, "s1", 9_000).unwrap();
        let trashed = SnippetRepo::new(&conn).get("s1").unwrap().unwrap();
        assert_eq!(trashed.deleted_at, Some(9_000));
        // Re-applying (or tombstoning a missing entity) stays successful.
        apply_tombstone(&conn, SyncEntityType::Snippet, "s1", 9_500).unwrap();
        apply_tombstone(&conn, SyncEntityType::Snippet, "ghost", 9_500).unwrap();
        assert_eq!(
            SnippetRepo::new(&conn)
                .get("s1")
                .unwrap()
                .unwrap()
                .deleted_at,
            Some(9_000)
        );
    }

    #[test]
    fn structural_tombstones_tolerate_absence() {
        let conn = db();
        apply_tombstone(&conn, SyncEntityType::Folder, "ghost", 1).unwrap();
        apply_tombstone(&conn, SyncEntityType::Tag, "ghost", 1).unwrap();
        apply_tombstone(&conn, SyncEntityType::AppRule, "ghost", 1).unwrap();
        apply_tombstone(&conn, SyncEntityType::TemplateField, "ghost", 1).unwrap();
    }

    #[test]
    fn ai_action_has_no_apply_path_yet() {
        let conn = db();
        assert!(matches!(
            build_document(&conn, SyncEntityType::AiAction, "a1"),
            Err(PayloadError::UnsupportedEntity)
        ));
        assert!(matches!(
            apply_document(&conn, SyncEntityType::AiAction, "a1", b"{}"),
            Err(PayloadError::UnsupportedEntity)
        ));
    }

    #[test]
    fn a_missing_entity_builds_no_document() {
        let conn = db();
        assert_eq!(
            build_document(&conn, SyncEntityType::Snippet, "ghost").unwrap(),
            None
        );
        assert_eq!(
            build_document(&conn, SyncEntityType::SnippetTag, "a/b").unwrap(),
            None
        );
    }
}

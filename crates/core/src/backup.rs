//! Versioned encrypted backup.
//!
//! A backup is a single text file:
//!
//! ```json
//! { "format": "typvia-backup", "version": 1, "kdf": {…}, "sealed": "base64" }
//! ```
//!
//! `sealed` is the whole library document (snippets incl. trash, folders,
//! tags, tag links, template fields, version history, and the vault's
//! *wrapped* key material) serialized as JSON and sealed with
//! XChaCha20-Poly1305 under a key derived from the user's backup passphrase
//! via Argon2id. Sensitive snippet
//! bodies stay in their vault envelopes inside the document — the backup
//! never holds sensitive plaintext at any layer, and restoring on a new
//! machine still requires the original master password to unlock the vault.
//!
//! Opening a backup with a wrong passphrase, a tampered file, or a damaged
//! file fails with the same error — the failure never says which.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};
use typvia_crypto::{KdfParams, SALT_LEN, SymmetricKey, derive_kek, open, seal};
use zeroize::Zeroizing;

use crate::model::{
    DomainKey, Folder, KeyDomain, Snippet, SnippetContent, SnippetVersion, Tag, TemplateField,
    VaultKeyHeader,
};

/// File-format discriminator.
pub const BACKUP_FORMAT: &str = "typvia-backup";

/// Current backup format version; readers reject newer versions with a
/// stable "made by a newer Typvia" error (forward-compatibility bit).
pub const BACKUP_VERSION: u32 = 1;

/// Upper bound on a backup file's text size (whole-library exports are far
/// smaller; the bound keeps hostile files from exhausting memory).
pub const BACKUP_MAX_BYTES: usize = 128 * 1024 * 1024;

/// AAD binding the sealed document to this format and version.
const BACKUP_AAD: &[u8] = b"typvia:backup:v1";

/// Why a backup could not be produced or read. Messages are static and never
/// echo file content or distinguish wrong-passphrase from tampering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupError {
    /// Not a Typvia backup file (structure or format marker wrong).
    InvalidFile,
    /// The file is bigger than [`BACKUP_MAX_BYTES`].
    TooLarge,
    /// Written by a newer Typvia than this one.
    UnsupportedVersion,
    /// Wrong passphrase, tampering, or damage — deliberately indistinct.
    CannotOpen,
    /// The decrypted document is not a valid library document.
    InvalidDocument,
}

impl BackupError {
    /// Stable, user-safe message.
    pub fn message(self) -> &'static str {
        match self {
            Self::InvalidFile => "not a Typvia backup file",
            Self::TooLarge => "the file is larger than the backup size limit",
            Self::UnsupportedVersion => "this backup was made by a newer version of Typvia",
            Self::CannotOpen => "the backup could not be opened with this passphrase",
            Self::InvalidDocument => "the backup content is not a valid library",
        }
    }
}

/// The whole-library document carried inside the sealed payload. Bytes are
/// base64 text; enums are their stable storage strings.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDocument {
    pub exported_at: i64,
    pub snippets: Vec<SnippetRecord>,
    pub folders: Vec<FolderRecord>,
    pub tags: Vec<TagRecord>,
    /// Snippet↔tag links, by id pair.
    pub tag_links: Vec<TagLinkRecord>,
    pub template_fields: Vec<TemplateFieldRecord>,
    pub versions: Vec<VersionRecord>,
    /// Wrapped vault key material (present once the vault is initialized).
    pub vault: Option<VaultRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetRecord {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub content_plaintext: Option<String>,
    /// base64; mutually exclusive with `content_plaintext`.
    pub content_ciphertext: Option<String>,
    pub snippet_type: String,
    pub description: Option<String>,
    pub folder_id: Option<String>,
    pub trigger: Option<String>,
    pub trigger_mode: Option<String>,
    pub language: Option<String>,
    pub security_level: String,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub is_enabled: bool,
    pub platform_scope: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_used_at: Option<i64>,
    pub usage_count: u64,
    pub version: u32,
    pub deleted_at: Option<i64>,
    /// Conflict-copy marker; defaulted so pre-0006 backups import.
    #[serde(default)]
    pub conflict_of: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderRecord {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagRecord {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagLinkRecord {
    pub snippet_id: String,
    pub tag_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateFieldRecord {
    pub id: String,
    pub snippet_id: String,
    pub name: String,
    pub label: String,
    pub field_type: String,
    pub default_value: Option<String>,
    pub options: Vec<String>,
    pub validation: Option<String>,
    pub is_required: bool,
    pub sort_order: i32,
    pub platform_overrides: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRecord {
    pub id: String,
    pub snippet_id: String,
    pub version: u32,
    pub title: String,
    pub content_plaintext: Option<String>,
    /// base64; mutually exclusive with `content_plaintext`.
    pub content_ciphertext: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultRecord {
    pub header: KeyHeaderRecord,
    pub domain_keys: Vec<DomainKeyRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyHeaderRecord {
    pub id: String,
    pub kdf: KdfRecord,
    /// base64 (MK sealed under the KEK — never plaintext key material).
    pub wrapped_mk: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainKeyRecord {
    pub domain: String,
    pub key_id: u32,
    /// base64 (domain key sealed under the MK).
    pub wrapped_key: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfRecord {
    pub version: u8,
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
    /// base64.
    pub salt: String,
}

/// The outer (cleartext) file structure: format marker, version, the KDF
/// parameters for the backup passphrase, and the sealed document.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupFile {
    format: String,
    version: u32,
    kdf: KdfRecord,
    /// base64 envelope over the serialized [`BackupDocument`].
    sealed: String,
}

/// Seals a library document under a key derived from `passphrase` (fresh
/// Argon2id salt per export) and returns the backup file text.
pub fn export(document: &BackupDocument, passphrase: &str) -> Result<String, BackupError> {
    let kdf = KdfParams::v1();
    let key = derive_backup_key(passphrase, &kdf)?;
    let plaintext =
        Zeroizing::new(serde_json::to_vec(document).map_err(|_| BackupError::InvalidDocument)?);
    let sealed = seal(&key, 0, BACKUP_AAD, &plaintext).map_err(|_| BackupError::CannotOpen)?;
    let file = BackupFile {
        format: BACKUP_FORMAT.to_string(),
        version: BACKUP_VERSION,
        kdf: kdf_record(&kdf),
        sealed: BASE64.encode(sealed),
    };
    serde_json::to_string(&file).map_err(|_| BackupError::InvalidDocument)
}

/// Opens a backup file's text with `passphrase` and returns the library
/// document. Wrong passphrase / tampering / damage all fail as
/// [`BackupError::CannotOpen`].
pub fn import(text: &str, passphrase: &str) -> Result<BackupDocument, BackupError> {
    if text.len() > BACKUP_MAX_BYTES {
        return Err(BackupError::TooLarge);
    }
    let file: BackupFile = serde_json::from_str(text).map_err(|_| BackupError::InvalidFile)?;
    if file.format != BACKUP_FORMAT {
        return Err(BackupError::InvalidFile);
    }
    if file.version > BACKUP_VERSION {
        return Err(BackupError::UnsupportedVersion);
    }
    let kdf = parse_kdf(&file.kdf)?;
    let key = derive_backup_key(passphrase, &kdf)?;
    let sealed = BASE64
        .decode(&file.sealed)
        .map_err(|_| BackupError::InvalidFile)?;
    let plaintext = open(&key, BACKUP_AAD, &sealed).map_err(|_| BackupError::CannotOpen)?;
    serde_json::from_slice(&plaintext).map_err(|_| BackupError::InvalidDocument)
}

fn derive_backup_key(passphrase: &str, kdf: &KdfParams) -> Result<SymmetricKey, BackupError> {
    let password = Zeroizing::new(passphrase.as_bytes().to_vec());
    derive_kek(&password, kdf).map_err(|_| BackupError::CannotOpen)
}

fn kdf_record(kdf: &KdfParams) -> KdfRecord {
    KdfRecord {
        version: kdf.version,
        m_cost_kib: kdf.m_cost_kib,
        t_cost: kdf.t_cost,
        p_cost: kdf.p_cost,
        salt: BASE64.encode(kdf.salt),
    }
}

fn parse_kdf(record: &KdfRecord) -> Result<KdfParams, BackupError> {
    let salt_bytes = BASE64
        .decode(&record.salt)
        .map_err(|_| BackupError::InvalidFile)?;
    let salt: [u8; SALT_LEN] = salt_bytes
        .as_slice()
        .try_into()
        .map_err(|_| BackupError::InvalidFile)?;
    Ok(KdfParams {
        version: record.version,
        m_cost_kib: record.m_cost_kib,
        t_cost: record.t_cost,
        p_cost: record.p_cost,
        salt,
    })
}

// --- Model ↔ record mapping -------------------------------------------------
//
// Records use stable storage strings for enums and base64 for bytes. The
// `to_model` direction validates (a backup is external input); repository
// inserts validate the models again before anything lands in storage.

fn content_pair(content: &SnippetContent) -> (Option<String>, Option<String>) {
    match content {
        SnippetContent::Plaintext(text) => (Some(text.clone()), None),
        SnippetContent::Ciphertext(bytes) => (None, Some(BASE64.encode(bytes))),
    }
}

fn parse_content(
    plaintext: &Option<String>,
    ciphertext: &Option<String>,
) -> Result<SnippetContent, BackupError> {
    match (plaintext, ciphertext) {
        (Some(text), None) => Ok(SnippetContent::Plaintext(text.clone())),
        (None, Some(encoded)) => Ok(SnippetContent::Ciphertext(
            BASE64
                .decode(encoded)
                .map_err(|_| BackupError::InvalidDocument)?,
        )),
        _ => Err(BackupError::InvalidDocument),
    }
}

impl SnippetRecord {
    pub fn from_model(snippet: &Snippet) -> Self {
        let (content_plaintext, content_ciphertext) = content_pair(&snippet.content);
        Self {
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

    pub fn to_model(&self) -> Result<Snippet, BackupError> {
        Ok(Snippet {
            id: self.id.clone(),
            workspace_id: self.workspace_id.clone(),
            title: self.title.clone(),
            content: parse_content(&self.content_plaintext, &self.content_ciphertext)?,
            snippet_type: self
                .snippet_type
                .parse()
                .map_err(|_| BackupError::InvalidDocument)?,
            description: self.description.clone(),
            folder_id: self.folder_id.clone(),
            trigger: self.trigger.clone(),
            trigger_mode: self
                .trigger_mode
                .as_deref()
                .map(str::parse)
                .transpose()
                .map_err(|_| BackupError::InvalidDocument)?,
            language: self.language.clone(),
            security_level: self
                .security_level
                .parse()
                .map_err(|_| BackupError::InvalidDocument)?,
            is_favorite: self.is_favorite,
            is_pinned: self.is_pinned,
            is_enabled: self.is_enabled,
            platform_scope: self
                .platform_scope
                .iter()
                .map(|p| p.parse().map_err(|_| BackupError::InvalidDocument))
                .collect::<Result<Vec<_>, _>>()?,
            created_at: self.created_at,
            updated_at: self.updated_at,
            last_used_at: self.last_used_at,
            usage_count: self.usage_count,
            version: self.version,
            deleted_at: self.deleted_at,
            conflict_of: self.conflict_of.clone(),
        })
    }
}

impl FolderRecord {
    pub fn from_model(folder: &Folder) -> Self {
        Self {
            id: folder.id.clone(),
            parent_id: folder.parent_id.clone(),
            name: folder.name.clone(),
            sort_order: folder.sort_order,
            created_at: folder.created_at,
            updated_at: folder.updated_at,
        }
    }

    pub fn to_model(&self) -> Folder {
        Folder {
            id: self.id.clone(),
            parent_id: self.parent_id.clone(),
            name: self.name.clone(),
            sort_order: self.sort_order,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl TagRecord {
    pub fn from_model(tag: &Tag) -> Self {
        Self {
            id: tag.id.clone(),
            name: tag.name.clone(),
            created_at: tag.created_at,
        }
    }

    pub fn to_model(&self) -> Tag {
        Tag {
            id: self.id.clone(),
            name: self.name.clone(),
            created_at: self.created_at,
        }
    }
}

impl TemplateFieldRecord {
    pub fn from_model(field: &TemplateField) -> Self {
        Self {
            id: field.id.clone(),
            snippet_id: field.snippet_id.clone(),
            name: field.name.clone(),
            label: field.label.clone(),
            field_type: field.field_type.as_str().to_string(),
            default_value: field.default_value.clone(),
            options: field.options.clone(),
            validation: field.validation.clone(),
            is_required: field.is_required,
            sort_order: field.sort_order,
            platform_overrides: field.platform_overrides.clone(),
        }
    }

    pub fn to_model(&self) -> Result<TemplateField, BackupError> {
        Ok(TemplateField {
            id: self.id.clone(),
            snippet_id: self.snippet_id.clone(),
            name: self.name.clone(),
            label: self.label.clone(),
            field_type: self
                .field_type
                .parse()
                .map_err(|_| BackupError::InvalidDocument)?,
            default_value: self.default_value.clone(),
            options: self.options.clone(),
            validation: self.validation.clone(),
            is_required: self.is_required,
            sort_order: self.sort_order,
            platform_overrides: self.platform_overrides.clone(),
        })
    }
}

impl VersionRecord {
    pub fn from_model(entry: &SnippetVersion) -> Self {
        let (content_plaintext, content_ciphertext) = content_pair(&entry.content);
        Self {
            id: entry.id.clone(),
            snippet_id: entry.snippet_id.clone(),
            version: entry.version,
            title: entry.title.clone(),
            content_plaintext,
            content_ciphertext,
            created_at: entry.created_at,
        }
    }

    pub fn to_model(&self) -> Result<SnippetVersion, BackupError> {
        Ok(SnippetVersion {
            id: self.id.clone(),
            snippet_id: self.snippet_id.clone(),
            version: self.version,
            title: self.title.clone(),
            content: parse_content(&self.content_plaintext, &self.content_ciphertext)?,
            created_at: self.created_at,
        })
    }
}

impl KeyHeaderRecord {
    pub fn from_model(header: &VaultKeyHeader) -> Self {
        Self {
            id: header.id.clone(),
            kdf: kdf_record(&header.kdf),
            wrapped_mk: BASE64.encode(&header.wrapped_mk),
            created_at: header.created_at,
            updated_at: header.updated_at,
        }
    }

    pub fn to_model(&self) -> Result<VaultKeyHeader, BackupError> {
        Ok(VaultKeyHeader {
            id: self.id.clone(),
            kdf: parse_kdf(&self.kdf).map_err(|_| BackupError::InvalidDocument)?,
            wrapped_mk: BASE64
                .decode(&self.wrapped_mk)
                .map_err(|_| BackupError::InvalidDocument)?,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

impl DomainKeyRecord {
    pub fn from_model(key: &DomainKey) -> Self {
        Self {
            domain: key.domain.as_str().to_string(),
            key_id: key.key_id,
            wrapped_key: BASE64.encode(&key.wrapped_key),
            created_at: key.created_at,
        }
    }

    pub fn to_model(&self) -> Result<DomainKey, BackupError> {
        Ok(DomainKey {
            domain: self
                .domain
                .parse::<KeyDomain>()
                .map_err(|_| BackupError::InvalidDocument)?,
            key_id: self.key_id,
            wrapped_key: BASE64
                .decode(&self.wrapped_key)
                .map_err(|_| BackupError::InvalidDocument)?,
            created_at: self.created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_document() -> BackupDocument {
        BackupDocument {
            exported_at: 1_000,
            snippets: vec![SnippetRecord {
                id: "s1".to_string(),
                workspace_id: "default".to_string(),
                title: "Greeting".to_string(),
                content_plaintext: Some("Hello".to_string()),
                content_ciphertext: None,
                snippet_type: "text".to_string(),
                description: None,
                folder_id: None,
                trigger: None,
                trigger_mode: None,
                language: None,
                security_level: "normal".to_string(),
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
            }],
            ..BackupDocument::default()
        }
    }

    #[test]
    fn export_import_round_trips_with_the_right_passphrase() {
        let text = export(&tiny_document(), "correct horse").expect("exports");
        let doc = import(&text, "correct horse").expect("imports");
        assert_eq!(doc.snippets.len(), 1);
        assert_eq!(doc.snippets[0].title, "Greeting");
        assert_eq!(doc.exported_at, 1_000);
    }

    #[test]
    fn a_wrong_passphrase_fails_without_saying_why() {
        let text = export(&tiny_document(), "correct horse").expect("exports");
        assert_eq!(
            import(&text, "wrong horse").unwrap_err(),
            BackupError::CannotOpen
        );
    }

    #[test]
    fn tampering_fails_the_same_way_as_a_wrong_passphrase() {
        let text = export(&tiny_document(), "pw").expect("exports");
        let mut file: serde_json::Value = serde_json::from_str(&text).unwrap();
        let sealed = file["sealed"].as_str().unwrap();
        let mut bytes = BASE64.decode(sealed).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;
        file["sealed"] = serde_json::Value::String(BASE64.encode(bytes));
        let err = import(&file.to_string(), "pw").unwrap_err();
        assert_eq!(err, BackupError::CannotOpen);
        assert_eq!(err.message(), BackupError::CannotOpen.message());
    }

    #[test]
    fn the_backup_bytes_never_contain_the_document_plaintext() {
        let text = export(&tiny_document(), "pw").expect("exports");
        assert!(!text.contains("Greeting"));
        assert!(!text.contains("Hello"));
    }

    #[test]
    fn newer_versions_are_rejected_with_the_forward_compat_error() {
        let text = export(&tiny_document(), "pw").expect("exports");
        let bumped = text.replace("\"version\":1", "\"version\":9");
        let err = import(&bumped, "pw").unwrap_err();
        assert_eq!(err, BackupError::UnsupportedVersion);
    }

    #[test]
    fn non_backup_files_are_rejected_as_invalid() {
        assert_eq!(import("{}", "pw").unwrap_err(), BackupError::InvalidFile);
        assert_eq!(
            import("not json at all", "pw").unwrap_err(),
            BackupError::InvalidFile
        );
        let other = r#"{"format":"other","version":1,"kdf":{"version":1,"mCostKib":8,"tCost":1,"pCost":1,"salt":"AAAAAAAAAAAAAAAAAAAAAA=="},"sealed":"AAAA"}"#;
        assert_eq!(import(other, "pw").unwrap_err(), BackupError::InvalidFile);
    }
}

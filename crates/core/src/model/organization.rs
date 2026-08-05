//! Organizational entities: `Folder`, `Tag`, `SnippetTag` (PRD §15.3–§15.5).

use super::validation::{ValidationError, require_non_blank};
use super::{FolderId, SnippetId, TagId, TimestampMs};

/// A folder in the snippet tree; folders nest via `parent_id` (PRD §15.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub id: FolderId,
    /// `None` for top-level folders.
    pub parent_id: Option<FolderId>,
    pub name: String,
    pub sort_order: i32,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

impl Folder {
    /// Validates the folder before it enters storage. Cycle detection across
    /// multiple folders is a repository concern; only self-reference is
    /// checkable on a single value.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_blank("name", &self.name)?;
        if self.parent_id.as_deref() == Some(self.id.as_str()) {
            return Err(ValidationError::new(
                "parent_id",
                "folder cannot be its own parent",
            ));
        }
        Ok(())
    }
}

/// A flat label attachable to snippets (PRD §15.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub id: TagId,
    pub name: String,
    pub created_at: TimestampMs,
}

impl Tag {
    /// Validates the tag before it enters storage.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_blank("name", &self.name)
    }
}

/// Snippet-to-tag association (PRD §15.5); the pair is the identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SnippetTag {
    pub snippet_id: SnippetId,
    pub tag_id: TagId,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn folder_rejects_being_its_own_parent() {
        let folder = Folder {
            id: "f1".to_string(),
            parent_id: Some("f1".to_string()),
            name: "Shell".to_string(),
            sort_order: 0,
            created_at: 0,
            updated_at: 0,
        };
        assert_eq!(folder.validate().unwrap_err().field, "parent_id");
    }

    #[test]
    fn folder_accepts_a_distinct_parent() {
        let folder = Folder {
            id: "f2".to_string(),
            parent_id: Some("f1".to_string()),
            name: "Docker".to_string(),
            sort_order: 1,
            created_at: 0,
            updated_at: 0,
        };
        assert_eq!(folder.validate(), Ok(()));
    }

    #[test]
    fn tag_rejects_blank_name() {
        let tag = Tag {
            id: "t1".to_string(),
            name: " ".to_string(),
            created_at: 0,
        };
        assert_eq!(tag.validate().unwrap_err().field, "name");
    }
}

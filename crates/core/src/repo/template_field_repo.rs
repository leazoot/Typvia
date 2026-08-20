//! TemplateField CRUD. A snippet's fields are edited as a whole
//! set by the Template Builder, so the write path is a transactional
//! replace-all rather than per-row mutation.

use rusqlite::{Connection, Row, params};

use super::{RepoError, new_id};
use crate::model::{TemplateField, TemplateFieldType};

/// TemplateField repository over a single connection.
pub struct TemplateFieldRepo<'c> {
    conn: &'c Connection,
}

impl<'c> TemplateFieldRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Lists a snippet's fields in display order (sort_order, then name).
    pub fn list_by_snippet(&self, snippet_id: &str) -> Result<Vec<TemplateField>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, snippet_id, name, label, type, default_value, options, \
             validation, is_required, sort_order, platform_overrides \
             FROM template_field WHERE snippet_id = ?1 \
             ORDER BY sort_order, name",
        )?;
        let rows = stmt.query_map(params![snippet_id], row_to_field)?;
        let mut fields = Vec::new();
        for row in rows {
            fields.push(row??);
        }
        Ok(fields)
    }

    /// Replaces the snippet's entire field set atomically: every field is
    /// validated, then the old set is deleted and the new set inserted.
    /// A field with a blank id is assigned a fresh UUID. Duplicate names within
    /// the set are a business conflict. On any failure the previous set stands.
    /// When the caller already holds a transaction (e.g. a backup restore),
    /// the replace runs inside it instead of opening a nested one.
    pub fn replace_for_snippet(
        &self,
        snippet_id: &str,
        fields: &[TemplateField],
    ) -> Result<Vec<TemplateField>, RepoError> {
        let mut stored: Vec<TemplateField> = Vec::with_capacity(fields.len());
        for (index, field) in fields.iter().enumerate() {
            field.validate()?;
            if field.snippet_id != snippet_id {
                return Err(RepoError::Conflict("field belongs to another snippet"));
            }
            let mut owned = field.clone();
            if owned.id.trim().is_empty() {
                owned.id = new_id();
            }
            owned.sort_order = i32::try_from(index).unwrap_or(owned.sort_order);
            stored.push(owned);
        }

        if self.conn.is_autocommit() {
            let tx = self.conn.unchecked_transaction()?;
            Self::write_set(&tx, snippet_id, &stored)?;
            tx.commit()?;
        } else {
            Self::write_set(self.conn, snippet_id, &stored)?;
        }
        Ok(stored)
    }

    /// Loads one field by id (sync document builder).
    pub fn get(&self, id: &str) -> Result<Option<TemplateField>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, snippet_id, name, label, type, default_value, options, \
             validation, is_required, sort_order, platform_overrides \
             FROM template_field WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_field(row)??)),
            None => Ok(None),
        }
    }

    /// Upserts one field by id (sync apply path): a pulled remote
    /// field lands whole without touching its siblings. A name collision
    /// with another field of the same snippet is a business conflict.
    pub fn upsert(&self, field: &TemplateField) -> Result<(), RepoError> {
        field.validate()?;
        match self.conn.execute(
            "INSERT INTO template_field \
             (id, snippet_id, name, label, type, default_value, options, \
              validation, is_required, sort_order, platform_overrides) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
             ON CONFLICT (id) DO UPDATE SET \
               snippet_id = excluded.snippet_id, name = excluded.name, \
               label = excluded.label, type = excluded.type, \
               default_value = excluded.default_value, options = excluded.options, \
               validation = excluded.validation, is_required = excluded.is_required, \
               sort_order = excluded.sort_order, \
               platform_overrides = excluded.platform_overrides",
            params![
                field.id,
                field.snippet_id,
                field.name,
                field.label,
                field.field_type.as_str(),
                field.default_value,
                options_to_json(&field.options),
                field.validation,
                i64::from(field.is_required),
                field.sort_order,
                field.platform_overrides,
            ],
        ) {
            Ok(_) => Ok(()),
            Err(e) if is_unique_violation(&e) => {
                Err(RepoError::Conflict("duplicate field name in template"))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Deletes one field by id (sync tombstone application).
    /// Deleting an absent field succeeds — the tombstone's goal state
    /// already holds.
    pub fn delete(&self, id: &str) -> Result<(), RepoError> {
        self.conn
            .execute("DELETE FROM template_field WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn write_set(
        conn: &Connection,
        snippet_id: &str,
        stored: &[TemplateField],
    ) -> Result<(), RepoError> {
        conn.execute(
            "DELETE FROM template_field WHERE snippet_id = ?1",
            params![snippet_id],
        )?;
        for field in stored {
            match conn.execute(
                "INSERT INTO template_field \
                 (id, snippet_id, name, label, type, default_value, options, \
                  validation, is_required, sort_order, platform_overrides) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    field.id,
                    field.snippet_id,
                    field.name,
                    field.label,
                    field.field_type.as_str(),
                    field.default_value,
                    options_to_json(&field.options),
                    field.validation,
                    i64::from(field.is_required),
                    field.sort_order,
                    field.platform_overrides,
                ],
            ) {
                Ok(_) => {}
                Err(e) if is_unique_violation(&e) => {
                    return Err(RepoError::Conflict("duplicate field name in template"));
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
}

fn row_to_field(row: &Row<'_>) -> rusqlite::Result<Result<TemplateField, RepoError>> {
    let type_text: String = row.get(4)?;
    let options_json: String = row.get(6)?;
    let is_required: i64 = row.get(8)?;

    let field_type = match type_text.parse::<TemplateFieldType>() {
        Ok(t) => t,
        Err(e) => return Ok(Err(RepoError::UnknownEnum(e))),
    };
    let options = match options_from_json(&options_json) {
        Ok(o) => o,
        Err(e) => return Ok(Err(e)),
    };

    Ok(Ok(TemplateField {
        id: row.get(0)?,
        snippet_id: row.get(1)?,
        name: row.get(2)?,
        label: row.get(3)?,
        field_type,
        default_value: row.get(5)?,
        options,
        validation: row.get(7)?,
        is_required: is_required != 0,
        sort_order: row.get(9)?,
        platform_overrides: row.get(10)?,
    }))
}

/// Serializes options to a JSON array TEXT.
fn options_to_json(options: &[String]) -> String {
    serde_json::to_string(options).unwrap_or_else(|_| "[]".to_string())
}

fn options_from_json(json: &str) -> Result<Vec<String>, RepoError> {
    serde_json::from_str(json).map_err(|_| RepoError::Conflict("corrupt template field options"))
}

fn is_unique_violation(e: &rusqlite::Error) -> bool {
    matches!(
        e.sqlite_error_code(),
        Some(rusqlite::ErrorCode::ConstraintViolation)
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::{migrate_to_latest, open_in_memory};
    use crate::model::{SecurityLevel, Snippet, SnippetContent, SnippetType};
    use crate::repo::SnippetRepo;

    fn db() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn seed_snippet(conn: &Connection, id: &str) {
        let repo = SnippetRepo::new(conn);
        let now = 1_700_000_000_000;
        repo.insert(&Snippet {
            id: id.to_string(),
            workspace_id: String::new(),
            title: "Bug report".to_string(),
            content: SnippetContent::Plaintext("### {{module}} {{severity}}".to_string()),
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
            created_at: now,
            updated_at: now,
            last_used_at: None,
            usage_count: 0,
            version: 1,
            deleted_at: None,
            conflict_of: None,
        })
        .unwrap();
    }

    fn field(snippet_id: &str, name: &str, ty: TemplateFieldType) -> TemplateField {
        TemplateField {
            id: String::new(),
            snippet_id: snippet_id.to_string(),
            name: name.to_string(),
            label: name.to_string(),
            field_type: ty,
            default_value: None,
            options: vec![],
            validation: None,
            is_required: true,
            sort_order: 0,
            platform_overrides: None,
        }
    }

    #[test]
    fn replace_assigns_ids_and_lists_in_order() {
        let conn = db();
        seed_snippet(&conn, "s1");
        {
            let repo = TemplateFieldRepo::new(&conn);
            let stored = repo
                .replace_for_snippet(
                    "s1",
                    &[
                        field("s1", "module", TemplateFieldType::SingleLineText),
                        field("s1", "severity", TemplateFieldType::MultiLineText),
                    ],
                )
                .unwrap();
            assert!(stored.iter().all(|f| !f.id.is_empty()));
            assert_eq!(stored[0].sort_order, 0);
            assert_eq!(stored[1].sort_order, 1);
        }
        let repo = TemplateFieldRepo::new(&conn);
        let listed = repo.list_by_snippet("s1").unwrap();
        assert_eq!(
            listed.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            vec!["module", "severity"]
        );
    }

    #[test]
    fn replace_is_idempotent_replacing_the_whole_set() {
        let conn = db();
        seed_snippet(&conn, "s1");
        let repo = TemplateFieldRepo::new(&conn);
        repo.replace_for_snippet(
            "s1",
            &[field("s1", "module", TemplateFieldType::SingleLineText)],
        )
        .unwrap();
        repo.replace_for_snippet(
            "s1",
            &[field("s1", "severity", TemplateFieldType::SingleLineText)],
        )
        .unwrap();
        let listed = repo.list_by_snippet("s1").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "severity");
    }

    #[test]
    fn duplicate_names_are_a_conflict_and_leave_the_old_set() {
        let conn = db();
        seed_snippet(&conn, "s1");
        let repo = TemplateFieldRepo::new(&conn);
        repo.replace_for_snippet(
            "s1",
            &[field("s1", "keep", TemplateFieldType::SingleLineText)],
        )
        .unwrap();
        let err = repo.replace_for_snippet(
            "s1",
            &[
                field("s1", "dup", TemplateFieldType::SingleLineText),
                field("s1", "dup", TemplateFieldType::SingleLineText),
            ],
        );
        assert!(matches!(err, Err(RepoError::Conflict(_))));
        // The prior set is intact (transaction rolled back).
        let listed = repo.list_by_snippet("s1").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "keep");
    }

    #[test]
    fn options_round_trip_through_storage() {
        let conn = db();
        seed_snippet(&conn, "s1");
        let repo = TemplateFieldRepo::new(&conn);
        let mut f = field("s1", "severity", TemplateFieldType::SingleSelect);
        f.options = vec!["low".to_string(), "high".to_string()];
        repo.replace_for_snippet("s1", &[f]).unwrap();
        let listed = repo.list_by_snippet("s1").unwrap();
        assert_eq!(listed[0].options, vec!["low", "high"]);
    }

    #[test]
    fn upsert_inserts_then_replaces_the_same_id() {
        let conn = db();
        seed_snippet(&conn, "s1");
        let repo = TemplateFieldRepo::new(&conn);
        let mut f = field("s1", "module", TemplateFieldType::SingleLineText);
        f.id = "tf-1".to_string();
        repo.upsert(&f).unwrap();
        f.label = "Module name".to_string();
        repo.upsert(&f).unwrap();
        let listed = repo.list_by_snippet("s1").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].label, "Module name");
    }

    #[test]
    fn upsert_rejects_a_name_collision_with_a_sibling() {
        let conn = db();
        seed_snippet(&conn, "s1");
        let repo = TemplateFieldRepo::new(&conn);
        let mut a = field("s1", "module", TemplateFieldType::SingleLineText);
        a.id = "tf-1".to_string();
        repo.upsert(&a).unwrap();
        let mut b = field("s1", "module", TemplateFieldType::SingleLineText);
        b.id = "tf-2".to_string();
        assert!(matches!(repo.upsert(&b), Err(RepoError::Conflict(_))));
    }

    #[test]
    fn delete_removes_a_field_and_tolerates_absence() {
        let conn = db();
        seed_snippet(&conn, "s1");
        let repo = TemplateFieldRepo::new(&conn);
        let mut f = field("s1", "module", TemplateFieldType::SingleLineText);
        f.id = "tf-1".to_string();
        repo.upsert(&f).unwrap();
        repo.delete("tf-1").unwrap();
        assert!(repo.list_by_snippet("s1").unwrap().is_empty());
        repo.delete("tf-1").unwrap();
    }

    #[test]
    fn rejects_a_field_for_another_snippet() {
        let conn = db();
        seed_snippet(&conn, "s1");
        let repo = TemplateFieldRepo::new(&conn);
        let err = repo.replace_for_snippet(
            "s1",
            &[field("other", "module", TemplateFieldType::SingleLineText)],
        );
        assert!(matches!(err, Err(RepoError::Conflict(_))));
    }
}

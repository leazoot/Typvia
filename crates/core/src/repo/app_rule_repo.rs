//! AppRule CRUD. Rules are per-snippet; evaluation semantics
//! live in [`crate::app_rules`].

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::RepoError;
use crate::model::{AppRule, Platform};

/// App-rule repository over a single connection.
pub struct AppRuleRepo<'c> {
    conn: &'c Connection,
}

const COLUMNS: &str = "id, snippet_id, platform, app_identifier, rule_type, window_title_pattern";

impl<'c> AppRuleRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Inserts a validated rule. A missing snippet is a business conflict
    /// (the FK would reject it anyway; the message names the actual rule).
    pub fn insert(&self, rule: &AppRule) -> Result<(), RepoError> {
        rule.validate()?;
        match self.conn.execute(
            "INSERT INTO app_rule (id, snippet_id, platform, app_identifier,
                                   rule_type, window_title_pattern)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                rule.id,
                rule.snippet_id,
                rule.platform.as_str(),
                rule.app_identifier,
                rule.rule_type.as_str(),
                rule.window_title_pattern,
            ],
        ) {
            Ok(_) => Ok(()),
            Err(e) if is_constraint_violation(&e) => {
                Err(RepoError::Conflict("rule id or snippet does not fit"))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Rewrites a rule's matching fields (the snippet binding is fixed).
    pub fn update(&self, rule: &AppRule) -> Result<(), RepoError> {
        rule.validate()?;
        let changed = self.conn.execute(
            "UPDATE app_rule SET platform = ?2, app_identifier = ?3,
                    rule_type = ?4, window_title_pattern = ?5
             WHERE id = ?1",
            params![
                rule.id,
                rule.platform.as_str(),
                rule.app_identifier,
                rule.rule_type.as_str(),
                rule.window_title_pattern,
            ],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Deletes one rule.
    pub fn delete(&self, id: &str) -> Result<(), RepoError> {
        let changed = self
            .conn
            .execute("DELETE FROM app_rule WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Loads one rule by id.
    pub fn get(&self, id: &str) -> Result<Option<AppRule>, RepoError> {
        self.conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM app_rule WHERE id = ?1"),
                params![id],
                row_to_rule,
            )
            .optional()?
            .transpose()
    }

    /// A snippet's rules, stable order (uses `idx_app_rule_snippet_id`).
    pub fn list_for_snippet(&self, snippet_id: &str) -> Result<Vec<AppRule>, RepoError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM app_rule WHERE snippet_id = ?1
             ORDER BY app_identifier, rule_type"
        ))?;
        let rows = stmt.query_map(params![snippet_id], row_to_rule)?;
        collect(rows)
    }

    /// One platform's rules, paged (the evaluation caller drains pages once
    /// per panel summon; rule counts stay small in practice).
    pub fn list_for_platform(
        &self,
        platform: Platform,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<AppRule>, RepoError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM app_rule WHERE platform = ?1
             ORDER BY app_identifier, rule_type, id LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = stmt.query_map(params![platform.as_str(), limit, offset], row_to_rule)?;
        collect(rows)
    }
}

fn is_constraint_violation(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

type RuleRowResult = Result<AppRule, RepoError>;

fn collect(
    rows: impl Iterator<Item = rusqlite::Result<RuleRowResult>>,
) -> Result<Vec<AppRule>, RepoError> {
    let mut rules = Vec::new();
    for row in rows {
        rules.push(row??);
    }
    Ok(rules)
}

fn row_to_rule(row: &Row<'_>) -> rusqlite::Result<RuleRowResult> {
    let platform: String = row.get("platform")?;
    let rule_type: String = row.get("rule_type")?;
    // Stored TEXT enums parse explicitly; unknown values surface as errors
    // instead of being silently dropped (forward-compat rule).
    let parsed = (|| -> RuleRowResult {
        Ok(AppRule {
            id: row.get("id")?,
            snippet_id: row.get("snippet_id")?,
            platform: platform.parse().map_err(RepoError::from)?,
            app_identifier: row.get("app_identifier")?,
            rule_type: rule_type.parse().map_err(RepoError::from)?,
            window_title_pattern: row.get("window_title_pattern")?,
        })
    })();
    match parsed {
        Ok(rule) => Ok(Ok(rule)),
        Err(RepoError::Sqlite(e)) => Err(e),
        Err(other) => Ok(Err(other)),
    }
}

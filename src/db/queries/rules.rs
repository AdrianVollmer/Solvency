use crate::models::rule::{NewRule, Rule, RuleActionType};
use rusqlite::{params, Connection, OptionalExtension};
use tracing::debug;

pub fn list_rules(conn: &Connection) -> rusqlite::Result<Vec<Rule>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, pattern, action_type, action_value, created_at, updated_at
         FROM rules
         ORDER BY name",
    )?;

    let rules = stmt
        .query_map([], |row| {
            let action_type_str: String = row.get(3)?;
            Ok(Rule {
                id: row.get(0)?,
                name: row.get(1)?,
                pattern: row.get(2)?,
                action_type: RuleActionType::parse(&action_type_str)
                    .unwrap_or(RuleActionType::AssignCategory),
                action_value: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rules)
}

pub fn get_rule(conn: &Connection, id: i64) -> rusqlite::Result<Option<Rule>> {
    conn.query_row(
        "SELECT id, name, pattern, action_type, action_value, created_at, updated_at
         FROM rules WHERE id = ?",
        [id],
        |row| {
            let action_type_str: String = row.get(3)?;
            Ok(Rule {
                id: row.get(0)?,
                name: row.get(1)?,
                pattern: row.get(2)?,
                action_type: RuleActionType::parse(&action_type_str)
                    .unwrap_or(RuleActionType::AssignCategory),
                action_value: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )
    .optional()
}

pub fn create_rule(conn: &Connection, rule: &NewRule) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO rules (name, pattern, action_type, action_value) VALUES (?, ?, ?, ?)",
        params![
            rule.name,
            rule.pattern,
            rule.action_type.as_str(),
            rule.action_value
        ],
    )?;
    let id = conn.last_insert_rowid();
    debug!(rule_id = id, name = %rule.name, pattern = %rule.pattern, "Created rule");
    Ok(id)
}

pub fn update_rule(conn: &Connection, id: i64, rule: &NewRule) -> rusqlite::Result<bool> {
    let rows = conn.execute(
        "UPDATE rules SET name = ?, pattern = ?, action_type = ?, action_value = ?, updated_at = datetime('now') WHERE id = ?",
        params![
            rule.name,
            rule.pattern,
            rule.action_type.as_str(),
            rule.action_value,
            id
        ],
    )?;
    if rows > 0 {
        debug!(rule_id = id, name = %rule.name, "Updated rule");
    }
    Ok(rows > 0)
}

pub fn delete_rule(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    let rows = conn.execute("DELETE FROM rules WHERE id = ?", [id])?;
    if rows > 0 {
        debug!(rule_id = id, "Deleted rule");
    }
    Ok(rows > 0)
}

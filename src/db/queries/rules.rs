use crate::models::rule::{NewRule, Rule, RuleActionType};
use rusqlite::{params, Connection, OptionalExtension};
use tracing::{debug, warn};

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

pub fn delete_all_rules(conn: &Connection) -> rusqlite::Result<usize> {
    let rows = conn.execute("DELETE FROM rules", [])?;
    warn!(count = rows, "Deleted all rules");
    Ok(rows)
}

/// Batch-assign a category to the given transaction IDs.
pub fn apply_rule_category(
    conn: &Connection,
    transaction_ids: &[i64],
    category_id: i64,
) -> rusqlite::Result<usize> {
    if transaction_ids.is_empty() {
        return Ok(0);
    }
    let placeholders: String = transaction_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "UPDATE transactions SET category_id = ?, updated_at = datetime('now') WHERE id IN ({})",
        placeholders
    );
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    params_vec.push(Box::new(category_id));
    for id in transaction_ids {
        params_vec.push(Box::new(*id));
    }
    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let rows = conn.execute(&sql, params_refs.as_slice())?;
    debug!(count = rows, category_id, "Applied rule: assigned category");
    Ok(rows)
}

/// Batch-add a tag to the given transaction IDs.
pub fn apply_rule_tag(
    conn: &Connection,
    transaction_ids: &[i64],
    tag_id: i64,
) -> rusqlite::Result<usize> {
    if transaction_ids.is_empty() {
        return Ok(0);
    }
    let mut count = 0usize;
    for tx_id in transaction_ids {
        count += conn.execute(
            "INSERT OR IGNORE INTO transaction_tags (transaction_id, tag_id) VALUES (?, ?)",
            params![tx_id, tag_id],
        )?;
    }
    debug!(count, tag_id, "Applied rule: assigned tag");
    Ok(count)
}

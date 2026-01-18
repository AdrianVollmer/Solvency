use rusqlite::{params, Connection};
use tracing::{debug, info};

use crate::error::AppResult;
use crate::models::{ImportRow, ImportSession, ImportStatus};
use crate::services::csv_parser::ParsedExpense;

// Session operations

pub fn create_session(conn: &Connection, id: &str) -> AppResult<ImportSession> {
    conn.execute(
        "INSERT INTO import_sessions (id, status) VALUES (?1, ?2)",
        params![id, ImportStatus::Parsing.as_str()],
    )?;
    info!(session_id = %id, "Created import session");
    get_session(conn, id)
}

pub fn get_session(conn: &Connection, id: &str) -> AppResult<ImportSession> {
    let mut stmt = conn.prepare(
        "SELECT id, status, total_rows, processed_rows, error_count, errors, created_at, updated_at
         FROM import_sessions WHERE id = ?1",
    )?;

    let session = stmt.query_row(params![id], |row| {
        let errors_json: Option<String> = row.get(5)?;
        let errors: Vec<String> = errors_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        Ok(ImportSession {
            id: row.get(0)?,
            status: row
                .get::<_, String>(1)?
                .parse()
                .unwrap_or(ImportStatus::Failed),
            total_rows: row.get(2)?,
            processed_rows: row.get(3)?,
            error_count: row.get(4)?,
            errors,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;

    Ok(session)
}

pub fn update_session_status(conn: &Connection, id: &str, status: ImportStatus) -> AppResult<()> {
    conn.execute(
        "UPDATE import_sessions SET status = ?2, updated_at = datetime('now') WHERE id = ?1",
        params![id, status.as_str()],
    )?;
    debug!(session_id = %id, status = %status.as_str(), "Updated import session status");
    Ok(())
}

pub fn update_session_progress(
    conn: &Connection,
    id: &str,
    total_rows: i64,
    processed_rows: i64,
) -> AppResult<()> {
    conn.execute(
        "UPDATE import_sessions SET total_rows = ?2, processed_rows = ?3, updated_at = datetime('now') WHERE id = ?1",
        params![id, total_rows, processed_rows],
    )?;
    Ok(())
}

pub fn update_session_errors(
    conn: &Connection,
    id: &str,
    error_count: i64,
    errors: &[String],
) -> AppResult<()> {
    let errors_json = serde_json::to_string(errors).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "UPDATE import_sessions SET error_count = ?2, errors = ?3, updated_at = datetime('now') WHERE id = ?1",
        params![id, error_count, errors_json],
    )?;
    Ok(())
}

pub fn increment_session_processed(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE import_sessions SET processed_rows = processed_rows + 1, updated_at = datetime('now') WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn increment_session_error_count(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE import_sessions SET error_count = error_count + 1, updated_at = datetime('now') WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn delete_session(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM import_sessions WHERE id = ?1", params![id])?;
    debug!(session_id = %id, "Deleted import session");
    Ok(())
}

pub fn cleanup_old_sessions(conn: &Connection, hours: i64) -> AppResult<usize> {
    let deleted = conn.execute(
        "DELETE FROM import_sessions WHERE created_at < datetime('now', ?1)",
        params![format!("-{} hours", hours)],
    )?;
    if deleted > 0 {
        info!(
            count = deleted,
            older_than_hours = hours,
            "Cleaned up old import sessions"
        );
    }
    Ok(deleted)
}

// Row operations

pub fn insert_row(
    conn: &Connection,
    session_id: &str,
    row_index: i64,
    data: &ParsedExpense,
) -> AppResult<i64> {
    let data_json = serde_json::to_string(data).unwrap();
    conn.execute(
        "INSERT INTO import_rows (session_id, row_index, data) VALUES (?1, ?2, ?3)",
        params![session_id, row_index, data_json],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_rows_paginated(
    conn: &Connection,
    session_id: &str,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<ImportRow>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.session_id, r.row_index, r.data, r.category_id, c.name, r.status, r.error
         FROM import_rows r
         LEFT JOIN categories c ON r.category_id = c.id
         WHERE r.session_id = ?1
         ORDER BY r.row_index
         LIMIT ?2 OFFSET ?3",
    )?;

    let rows = stmt
        .query_map(params![session_id, limit, offset], |row| {
            let data_json: String = row.get(3)?;
            let data: ParsedExpense =
                serde_json::from_str(&data_json).unwrap_or_else(|_| ParsedExpense {
                    date: String::new(),
                    amount: String::new(),
                    currency: "USD".to_string(),
                    description: String::new(),
                    category: None,
                    tags: vec![],
                    notes: None,
                    value_date: None,
                    payer: None,
                    payee: None,
                    reference: None,
                    transaction_type: None,
                    counterparty_iban: None,
                    creditor_id: None,
                    mandate_reference: None,
                    customer_reference: None,
                    row_number: 0,
                });

            Ok(ImportRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                row_index: row.get(2)?,
                data,
                category_id: row.get(4)?,
                category_name: row.get(5)?,
                status: row.get(6)?,
                error: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

pub fn count_rows(conn: &Connection, session_id: &str) -> AppResult<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM import_rows WHERE session_id = ?1",
        params![session_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn get_pending_rows(conn: &Connection, session_id: &str) -> AppResult<Vec<ImportRow>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.session_id, r.row_index, r.data, r.category_id, c.name, r.status, r.error
         FROM import_rows r
         LEFT JOIN categories c ON r.category_id = c.id
         WHERE r.session_id = ?1 AND r.status = 'pending'
         ORDER BY r.row_index",
    )?;

    let rows = stmt
        .query_map(params![session_id], |row| {
            let data_json: String = row.get(3)?;
            let data: ParsedExpense =
                serde_json::from_str(&data_json).unwrap_or_else(|_| ParsedExpense {
                    date: String::new(),
                    amount: String::new(),
                    currency: "USD".to_string(),
                    description: String::new(),
                    category: None,
                    tags: vec![],
                    notes: None,
                    value_date: None,
                    payer: None,
                    payee: None,
                    reference: None,
                    transaction_type: None,
                    counterparty_iban: None,
                    creditor_id: None,
                    mandate_reference: None,
                    customer_reference: None,
                    row_number: 0,
                });

            Ok(ImportRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                row_index: row.get(2)?,
                data,
                category_id: row.get(4)?,
                category_name: row.get(5)?,
                status: row.get(6)?,
                error: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

pub fn update_row_category(
    conn: &Connection,
    row_id: i64,
    category_id: Option<i64>,
) -> AppResult<()> {
    conn.execute(
        "UPDATE import_rows SET category_id = ?2 WHERE id = ?1",
        params![row_id, category_id],
    )?;
    Ok(())
}

pub fn update_all_rows_category(
    conn: &Connection,
    session_id: &str,
    category_id: Option<i64>,
) -> AppResult<usize> {
    let updated = conn.execute(
        "UPDATE import_rows SET category_id = ?2 WHERE session_id = ?1 AND status = 'pending'",
        params![session_id, category_id],
    )?;
    debug!(session_id = %session_id, count = updated, "Updated category for all import rows");
    Ok(updated)
}

pub fn mark_row_imported(conn: &Connection, row_id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE import_rows SET status = 'imported' WHERE id = ?1",
        params![row_id],
    )?;
    Ok(())
}

pub fn mark_row_error(conn: &Connection, row_id: i64, error: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE import_rows SET status = 'error', error = ?2 WHERE id = ?1",
        params![row_id, error],
    )?;
    Ok(())
}

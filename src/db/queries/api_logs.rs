use crate::models::api_log::{ApiLog, NewApiLog};
use rusqlite::{params, Connection, OptionalExtension};

/// Insert a new API log entry
pub fn insert_api_log(conn: &Connection, log: &NewApiLog) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO api_logs (api_name, action, symbol, request_params, status, response_summary, response_details, duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            log.api_name,
            log.action,
            log.symbol,
            log.request_params,
            log.status,
            log.response_summary,
            log.response_details,
            log.duration_ms,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Get all API logs, most recent first
pub fn get_all_logs(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<ApiLog>> {
    let mut stmt = conn.prepare(
        "SELECT id, api_name, action, symbol, request_params, status, response_summary, response_details, duration_ms, created_at
         FROM api_logs
         ORDER BY created_at DESC
         LIMIT ?1",
    )?;

    let logs = stmt
        .query_map([limit], |row| {
            Ok(ApiLog {
                id: row.get(0)?,
                api_name: row.get(1)?,
                action: row.get(2)?,
                symbol: row.get(3)?,
                request_params: row.get(4)?,
                status: row.get(5)?,
                response_summary: row.get(6)?,
                response_details: row.get(7)?,
                duration_ms: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(logs)
}

/// Get a single log entry by ID
pub fn get_log_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<ApiLog>> {
    conn.query_row(
        "SELECT id, api_name, action, symbol, request_params, status, response_summary, response_details, duration_ms, created_at
         FROM api_logs WHERE id = ?1",
        [id],
        |row| {
            Ok(ApiLog {
                id: row.get(0)?,
                api_name: row.get(1)?,
                action: row.get(2)?,
                symbol: row.get(3)?,
                request_params: row.get(4)?,
                status: row.get(5)?,
                response_summary: row.get(6)?,
                response_details: row.get(7)?,
                duration_ms: row.get(8)?,
                created_at: row.get(9)?,
            })
        },
    )
    .optional()
}

/// Get failed logs since a given ID (for polling)
pub fn get_failed_logs_since(conn: &Connection, since_id: i64) -> rusqlite::Result<Vec<ApiLog>> {
    let mut stmt = conn.prepare(
        "SELECT id, api_name, action, symbol, request_params, status, response_summary, response_details, duration_ms, created_at
         FROM api_logs
         WHERE id > ?1 AND status = 'error'
         ORDER BY id ASC",
    )?;

    let logs = stmt
        .query_map([since_id], |row| {
            Ok(ApiLog {
                id: row.get(0)?,
                api_name: row.get(1)?,
                action: row.get(2)?,
                symbol: row.get(3)?,
                request_params: row.get(4)?,
                status: row.get(5)?,
                response_summary: row.get(6)?,
                response_details: row.get(7)?,
                duration_ms: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(logs)
}

/// Get the latest log ID (for initializing polling)
pub fn get_latest_log_id(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COALESCE(MAX(id), 0) FROM api_logs", [], |row| {
        row.get(0)
    })
}

use rusqlite::{params, Connection};
use tracing::{debug, info};

use crate::error::AppResult;
use crate::models::ai_categorization::{
    AiCategorizationResult, AiCategorizationResultWithDetails, AiCategorizationSession,
    AiCategorizationStatus, AiResultStatus,
};

// Session operations

pub fn create_session(
    conn: &Connection,
    id: &str,
    provider: &str,
    model: &str,
    total_transactions: i64,
) -> AppResult<AiCategorizationSession> {
    conn.execute(
        "INSERT INTO ai_categorization_sessions (id, status, provider, model, total_transactions)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id,
            AiCategorizationStatus::Pending.as_str(),
            provider,
            model,
            total_transactions
        ],
    )?;
    info!(session_id = %id, provider = %provider, model = %model, total = total_transactions, "Created AI categorization session");
    get_session(conn, id)
}

pub fn get_session(conn: &Connection, id: &str) -> AppResult<AiCategorizationSession> {
    let mut stmt = conn.prepare(
        "SELECT id, status, provider, model, total_transactions, processed_transactions,
                categorized_count, skipped_count, error_count, errors, created_at, updated_at
         FROM ai_categorization_sessions WHERE id = ?1",
    )?;

    let session = stmt.query_row(params![id], |row| {
        let errors_json: Option<String> = row.get(9)?;
        let errors: Vec<String> = errors_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        Ok(AiCategorizationSession {
            id: row.get(0)?,
            status: row
                .get::<_, String>(1)?
                .parse()
                .unwrap_or(AiCategorizationStatus::Failed),
            provider: row.get(2)?,
            model: row.get(3)?,
            total_transactions: row.get(4)?,
            processed_transactions: row.get(5)?,
            categorized_count: row.get(6)?,
            skipped_count: row.get(7)?,
            error_count: row.get(8)?,
            errors,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    })?;

    Ok(session)
}

pub fn update_session_status(
    conn: &Connection,
    id: &str,
    status: AiCategorizationStatus,
) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_categorization_sessions SET status = ?2, updated_at = datetime('now') WHERE id = ?1",
        params![id, status.as_str()],
    )?;
    info!(session_id = %id, status = %status.as_str(), "Updated AI categorization session status");
    Ok(())
}

pub fn update_session_progress(
    conn: &Connection,
    id: &str,
    processed: i64,
    categorized: i64,
    skipped: i64,
    errors: i64,
) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_categorization_sessions
         SET processed_transactions = ?2, categorized_count = ?3, skipped_count = ?4, error_count = ?5,
             updated_at = datetime('now')
         WHERE id = ?1",
        params![id, processed, categorized, skipped, errors],
    )?;
    Ok(())
}

pub fn update_session_errors(conn: &Connection, id: &str, errors: &[String]) -> AppResult<()> {
    let errors_json = serde_json::to_string(errors).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "UPDATE ai_categorization_sessions SET errors = ?2, updated_at = datetime('now') WHERE id = ?1",
        params![id, errors_json],
    )?;
    Ok(())
}

pub fn delete_session(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM ai_categorization_sessions WHERE id = ?1",
        params![id],
    )?;
    debug!(session_id = %id, "Deleted AI categorization session");
    Ok(())
}

// Result operations

pub fn insert_result(
    conn: &Connection,
    session_id: &str,
    transaction_id: i64,
    original_category_id: Option<i64>,
    suggested_category_id: Option<i64>,
    confidence: Option<f64>,
    ai_reasoning: Option<&str>,
    status: AiResultStatus,
    error: Option<&str>,
) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO ai_categorization_results
         (session_id, transaction_id, original_category_id, suggested_category_id, confidence, ai_reasoning, status, error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            session_id,
            transaction_id,
            original_category_id,
            suggested_category_id,
            confidence,
            ai_reasoning,
            status.as_str(),
            error
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_result(conn: &Connection, id: i64) -> AppResult<Option<AiCategorizationResult>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, transaction_id, original_category_id, suggested_category_id,
                confidence, ai_reasoning, status, error, created_at
         FROM ai_categorization_results WHERE id = ?1",
    )?;

    let result = stmt
        .query_row(params![id], |row| {
            Ok(AiCategorizationResult {
                id: row.get(0)?,
                session_id: row.get(1)?,
                transaction_id: row.get(2)?,
                original_category_id: row.get(3)?,
                suggested_category_id: row.get(4)?,
                confidence: row.get(5)?,
                ai_reasoning: row.get(6)?,
                status: row
                    .get::<_, String>(7)?
                    .parse()
                    .unwrap_or(AiResultStatus::Error),
                error: row.get(8)?,
                created_at: row.get(9)?,
            })
        })
        .ok();

    Ok(result)
}

pub fn get_results_with_details(
    conn: &Connection,
    session_id: &str,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<AiCategorizationResultWithDetails>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.session_id, r.transaction_id, r.original_category_id, r.suggested_category_id,
                r.confidence, r.ai_reasoning, r.status, r.error, r.created_at,
                t.date, t.description, t.amount_cents, t.currency,
                oc.name as original_category_name,
                sc.name as suggested_category_name
         FROM ai_categorization_results r
         JOIN transactions t ON r.transaction_id = t.id
         LEFT JOIN categories oc ON r.original_category_id = oc.id
         LEFT JOIN categories sc ON r.suggested_category_id = sc.id
         WHERE r.session_id = ?1
         ORDER BY r.id
         LIMIT ?2 OFFSET ?3",
    )?;

    let rows = stmt
        .query_map(params![session_id, limit, offset], |row| {
            Ok(AiCategorizationResultWithDetails {
                result: AiCategorizationResult {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    transaction_id: row.get(2)?,
                    original_category_id: row.get(3)?,
                    suggested_category_id: row.get(4)?,
                    confidence: row.get(5)?,
                    ai_reasoning: row.get(6)?,
                    status: row
                        .get::<_, String>(7)?
                        .parse()
                        .unwrap_or(AiResultStatus::Error),
                    error: row.get(8)?,
                    created_at: row.get(9)?,
                },
                transaction_date: row.get(10)?,
                transaction_description: row.get(11)?,
                transaction_amount_cents: row.get(12)?,
                transaction_currency: row.get(13)?,
                original_category_name: row.get(14)?,
                suggested_category_name: row.get(15)?,
                suggested_category_path: None, // Will be filled in if needed
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

pub fn get_pending_results_with_details(
    conn: &Connection,
    session_id: &str,
) -> AppResult<Vec<AiCategorizationResultWithDetails>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.session_id, r.transaction_id, r.original_category_id, r.suggested_category_id,
                r.confidence, r.ai_reasoning, r.status, r.error, r.created_at,
                t.date, t.description, t.amount_cents, t.currency,
                oc.name as original_category_name,
                sc.name as suggested_category_name
         FROM ai_categorization_results r
         JOIN transactions t ON r.transaction_id = t.id
         LEFT JOIN categories oc ON r.original_category_id = oc.id
         LEFT JOIN categories sc ON r.suggested_category_id = sc.id
         WHERE r.session_id = ?1 AND r.status = 'pending'
         ORDER BY r.confidence DESC, r.id",
    )?;

    let rows = stmt
        .query_map(params![session_id], |row| {
            Ok(AiCategorizationResultWithDetails {
                result: AiCategorizationResult {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    transaction_id: row.get(2)?,
                    original_category_id: row.get(3)?,
                    suggested_category_id: row.get(4)?,
                    confidence: row.get(5)?,
                    ai_reasoning: row.get(6)?,
                    status: row
                        .get::<_, String>(7)?
                        .parse()
                        .unwrap_or(AiResultStatus::Error),
                    error: row.get(8)?,
                    created_at: row.get(9)?,
                },
                transaction_date: row.get(10)?,
                transaction_description: row.get(11)?,
                transaction_amount_cents: row.get(12)?,
                transaction_currency: row.get(13)?,
                original_category_name: row.get(14)?,
                suggested_category_name: row.get(15)?,
                suggested_category_path: None,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

pub fn count_results(conn: &Connection, session_id: &str) -> AppResult<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ai_categorization_results WHERE session_id = ?1",
        params![session_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn count_pending_results(conn: &Connection, session_id: &str) -> AppResult<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ai_categorization_results WHERE session_id = ?1 AND status = 'pending'",
        params![session_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn update_result_status(conn: &Connection, id: i64, status: AiResultStatus) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_categorization_results SET status = ?2 WHERE id = ?1",
        params![id, status.as_str()],
    )?;
    Ok(())
}

pub fn apply_result(conn: &Connection, result_id: i64) -> AppResult<bool> {
    // Get the result to find the suggested category
    let result = match get_result(conn, result_id)? {
        Some(r) => r,
        None => return Ok(false),
    };

    if let Some(category_id) = result.suggested_category_id {
        // Update the transaction's category
        conn.execute(
            "UPDATE transactions SET category_id = ?2, updated_at = datetime('now') WHERE id = ?1",
            params![result.transaction_id, category_id],
        )?;

        // Mark result as applied
        update_result_status(conn, result_id, AiResultStatus::Applied)?;
        return Ok(true);
    }

    Ok(false)
}

pub fn apply_all_pending_results(conn: &Connection, session_id: &str) -> AppResult<i64> {
    // Get all pending results with suggestions
    let mut stmt = conn.prepare(
        "SELECT id, transaction_id, suggested_category_id
         FROM ai_categorization_results
         WHERE session_id = ?1 AND status = 'pending' AND suggested_category_id IS NOT NULL",
    )?;

    let results: Vec<(i64, i64, i64)> = stmt
        .query_map(params![session_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut applied = 0i64;
    for (result_id, transaction_id, category_id) in results {
        conn.execute(
            "UPDATE transactions SET category_id = ?2, updated_at = datetime('now') WHERE id = ?1",
            params![transaction_id, category_id],
        )?;
        update_result_status(conn, result_id, AiResultStatus::Applied)?;
        applied += 1;
    }

    info!(session_id = %session_id, applied = applied, "Applied all pending AI categorization results");
    Ok(applied)
}

pub fn reject_result(conn: &Connection, result_id: i64) -> AppResult<()> {
    update_result_status(conn, result_id, AiResultStatus::Rejected)
}

use crate::error::AppResult;
use crate::models::trading::{
    NewTradingActivity, Position, TradingActivity, TradingActivityType, TradingImportRow,
    TradingImportSession, TradingImportStatus,
};
use crate::services::trading_csv_parser::ParsedTradingActivity;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

/// Raw activity row for position calculations: (symbol, activity_type, quantity, unit_price_cents, fee_cents, currency)
type ActivityRow = (String, String, Option<f64>, Option<i64>, i64, String);

// Activity operations

#[derive(Default)]
pub struct TradingActivityFilter {
    pub symbol: Option<String>,
    pub activity_type: Option<TradingActivityType>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn list_activities(
    conn: &Connection,
    filter: &TradingActivityFilter,
) -> rusqlite::Result<Vec<TradingActivity>> {
    let mut sql = String::from(
        "SELECT id, date, symbol, quantity, activity_type, unit_price_cents,
                currency, fee_cents, notes, created_at, updated_at
         FROM trading_activities
         WHERE 1=1",
    );
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref symbol) = filter.symbol {
        sql.push_str(" AND symbol = ?");
        params_vec.push(Box::new(symbol.clone()));
    }
    if let Some(ref activity_type) = filter.activity_type {
        sql.push_str(" AND activity_type = ?");
        params_vec.push(Box::new(activity_type.as_str().to_string()));
    }
    if let Some(ref from_date) = filter.from_date {
        sql.push_str(" AND date >= ?");
        params_vec.push(Box::new(from_date.clone()));
    }
    if let Some(ref to_date) = filter.to_date {
        sql.push_str(" AND date <= ?");
        params_vec.push(Box::new(to_date.clone()));
    }

    sql.push_str(" ORDER BY date DESC, id DESC");

    if let Some(limit) = filter.limit {
        sql.push_str(" LIMIT ?");
        params_vec.push(Box::new(limit));
    }
    if let Some(offset) = filter.offset {
        sql.push_str(" OFFSET ?");
        params_vec.push(Box::new(offset));
    }

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;

    let activities = stmt
        .query_map(params_refs.as_slice(), |row| {
            let activity_type_str: String = row.get(4)?;
            Ok(TradingActivity {
                id: row.get(0)?,
                date: row.get(1)?,
                symbol: row.get(2)?,
                quantity: row.get(3)?,
                activity_type: activity_type_str
                    .parse()
                    .unwrap_or(TradingActivityType::Buy),
                unit_price_cents: row.get(5)?,
                currency: row.get(6)?,
                fee_cents: row.get(7)?,
                notes: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?
        .filter_map(|a| a.ok())
        .collect();

    Ok(activities)
}

pub fn count_activities(
    conn: &Connection,
    filter: &TradingActivityFilter,
) -> rusqlite::Result<i64> {
    let mut sql = String::from("SELECT COUNT(*) FROM trading_activities WHERE 1=1");
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref symbol) = filter.symbol {
        sql.push_str(" AND symbol = ?");
        params_vec.push(Box::new(symbol.clone()));
    }
    if let Some(ref activity_type) = filter.activity_type {
        sql.push_str(" AND activity_type = ?");
        params_vec.push(Box::new(activity_type.as_str().to_string()));
    }
    if let Some(ref from_date) = filter.from_date {
        sql.push_str(" AND date >= ?");
        params_vec.push(Box::new(from_date.clone()));
    }
    if let Some(ref to_date) = filter.to_date {
        sql.push_str(" AND date <= ?");
        params_vec.push(Box::new(to_date.clone()));
    }

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0))
}

pub fn get_activity(conn: &Connection, id: i64) -> rusqlite::Result<Option<TradingActivity>> {
    conn.query_row(
        "SELECT id, date, symbol, quantity, activity_type, unit_price_cents,
                currency, fee_cents, notes, created_at, updated_at
         FROM trading_activities WHERE id = ?",
        [id],
        |row| {
            let activity_type_str: String = row.get(4)?;
            Ok(TradingActivity {
                id: row.get(0)?,
                date: row.get(1)?,
                symbol: row.get(2)?,
                quantity: row.get(3)?,
                activity_type: activity_type_str
                    .parse()
                    .unwrap_or(TradingActivityType::Buy),
                unit_price_cents: row.get(5)?,
                currency: row.get(6)?,
                fee_cents: row.get(7)?,
                notes: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        },
    )
    .optional()
}

pub fn create_activity(conn: &Connection, activity: &NewTradingActivity) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO trading_activities (date, symbol, quantity, activity_type, unit_price_cents, currency, fee_cents, notes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            activity.date,
            activity.symbol,
            activity.quantity,
            activity.activity_type.as_str(),
            activity.unit_price_cents,
            activity.currency,
            activity.fee_cents,
            activity.notes,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_activity(
    conn: &Connection,
    id: i64,
    activity: &NewTradingActivity,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE trading_activities SET date = ?, symbol = ?, quantity = ?, activity_type = ?,
         unit_price_cents = ?, currency = ?, fee_cents = ?, notes = ?, updated_at = datetime('now')
         WHERE id = ?",
        params![
            activity.date,
            activity.symbol,
            activity.quantity,
            activity.activity_type.as_str(),
            activity.unit_price_cents,
            activity.currency,
            activity.fee_cents,
            activity.notes,
            id,
        ],
    )?;
    Ok(())
}

pub fn delete_activity(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    let rows = conn.execute("DELETE FROM trading_activities WHERE id = ?", [id])?;
    Ok(rows > 0)
}

// Position calculations

pub fn get_positions(conn: &Connection) -> rusqlite::Result<Vec<Position>> {
    // Get all activities grouped by symbol
    let mut stmt = conn.prepare(
        "SELECT symbol, activity_type, quantity, unit_price_cents, fee_cents, currency
         FROM trading_activities
         ORDER BY symbol, date ASC, id ASC",
    )?;

    let activities: Vec<ActivityRow> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Calculate positions by symbol
    let mut positions_map: HashMap<String, (f64, i64, String)> = HashMap::new();

    for (symbol, activity_type_str, quantity, unit_price_cents, _fee_cents, currency) in activities
    {
        let activity_type: TradingActivityType = activity_type_str
            .parse()
            .unwrap_or(TradingActivityType::Buy);
        let qty = quantity.unwrap_or(0.0);
        let price = unit_price_cents.unwrap_or(0);

        let entry = positions_map
            .entry(symbol.clone())
            .or_insert((0.0, 0, currency));

        match activity_type {
            TradingActivityType::Buy | TradingActivityType::AddHolding => {
                let cost = (qty * price as f64).round() as i64;
                entry.0 += qty;
                entry.1 += cost;
            }
            TradingActivityType::Sell | TradingActivityType::RemoveHolding => {
                // For sells, reduce quantity and proportionally reduce cost basis
                if entry.0 > 0.0 {
                    let avg_cost = entry.1 as f64 / entry.0;
                    let cost_reduction = (qty * avg_cost).round() as i64;
                    entry.0 -= qty;
                    entry.1 -= cost_reduction;
                    if entry.0 < 0.0 {
                        entry.0 = 0.0;
                    }
                    if entry.1 < 0 {
                        entry.1 = 0;
                    }
                }
            }
            TradingActivityType::TransferIn => {
                let cost = (qty * price as f64).round() as i64;
                entry.0 += qty;
                entry.1 += cost;
            }
            TradingActivityType::TransferOut => {
                if entry.0 > 0.0 {
                    let avg_cost = entry.1 as f64 / entry.0;
                    let cost_reduction = (qty * avg_cost).round() as i64;
                    entry.0 -= qty;
                    entry.1 -= cost_reduction;
                    if entry.0 < 0.0 {
                        entry.0 = 0.0;
                    }
                    if entry.1 < 0 {
                        entry.1 = 0;
                    }
                }
            }
            TradingActivityType::Split => {
                // Split adjusts quantity but not total cost
                // quantity field contains the split ratio (e.g., 2.0 for 2:1 split)
                if qty > 0.0 {
                    entry.0 *= qty;
                }
            }
            TradingActivityType::Deposit => {
                // Cash deposit - adds to cash position
                let amount = (qty * price as f64).round() as i64;
                entry.0 += qty;
                entry.1 += amount;
            }
            TradingActivityType::Withdrawal
            | TradingActivityType::Fee
            | TradingActivityType::Tax => {
                // These reduce cash
                let amount = (qty * price as f64).round() as i64;
                entry.0 -= qty;
                entry.1 -= amount;
            }
            TradingActivityType::Dividend | TradingActivityType::Interest => {
                // These add cash
                let amount = (qty * price as f64).round() as i64;
                entry.0 += qty;
                entry.1 += amount;
            }
        }
    }

    // Convert to Position structs, filtering out zero positions
    let mut positions: Vec<Position> = positions_map
        .into_iter()
        .filter(|(_, (qty, _, _))| *qty != 0.0)
        .map(
            |(symbol, (quantity, total_cost_cents, currency))| Position {
                symbol,
                quantity,
                total_cost_cents,
                currency,
            },
        )
        .collect();

    // Sort: cash positions first, then alphabetically
    positions.sort_by(|a, b| {
        let a_is_cash = a.is_cash();
        let b_is_cash = b.is_cash();
        match (a_is_cash, b_is_cash) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.symbol.cmp(&b.symbol),
        }
    });

    Ok(positions)
}

pub fn get_unique_symbols(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT DISTINCT symbol FROM trading_activities ORDER BY symbol")?;

    let symbols: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(symbols)
}

// Import session operations

pub fn create_import_session(conn: &Connection, id: &str) -> AppResult<TradingImportSession> {
    conn.execute(
        "INSERT INTO trading_import_sessions (id, status) VALUES (?1, ?2)",
        params![id, TradingImportStatus::Parsing.as_str()],
    )?;
    get_import_session(conn, id)
}

pub fn get_import_session(conn: &Connection, id: &str) -> AppResult<TradingImportSession> {
    let mut stmt = conn.prepare(
        "SELECT id, status, total_rows, processed_rows, error_count, errors, created_at, updated_at
         FROM trading_import_sessions WHERE id = ?1",
    )?;

    let session = stmt.query_row(params![id], |row| {
        let errors_json: Option<String> = row.get(5)?;
        let errors: Vec<String> = errors_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        Ok(TradingImportSession {
            id: row.get(0)?,
            status: row
                .get::<_, String>(1)?
                .parse()
                .unwrap_or(TradingImportStatus::Failed),
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

pub fn update_import_session_status(
    conn: &Connection,
    id: &str,
    status: TradingImportStatus,
) -> AppResult<()> {
    conn.execute(
        "UPDATE trading_import_sessions SET status = ?2, updated_at = datetime('now') WHERE id = ?1",
        params![id, status.as_str()],
    )?;
    Ok(())
}

pub fn update_import_session_progress(
    conn: &Connection,
    id: &str,
    total_rows: i64,
    processed_rows: i64,
) -> AppResult<()> {
    conn.execute(
        "UPDATE trading_import_sessions SET total_rows = ?2, processed_rows = ?3, updated_at = datetime('now') WHERE id = ?1",
        params![id, total_rows, processed_rows],
    )?;
    Ok(())
}

pub fn update_import_session_errors(
    conn: &Connection,
    id: &str,
    error_count: i64,
    errors: &[String],
) -> AppResult<()> {
    let errors_json = serde_json::to_string(errors).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "UPDATE trading_import_sessions SET error_count = ?2, errors = ?3, updated_at = datetime('now') WHERE id = ?1",
        params![id, error_count, errors_json],
    )?;
    Ok(())
}

pub fn increment_import_session_processed(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE trading_import_sessions SET processed_rows = processed_rows + 1, updated_at = datetime('now') WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn increment_import_session_error_count(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE trading_import_sessions SET error_count = error_count + 1, updated_at = datetime('now') WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn delete_import_session(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM trading_import_sessions WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

// Import row operations

pub fn insert_import_row(
    conn: &Connection,
    session_id: &str,
    row_index: i64,
    data: &ParsedTradingActivity,
) -> AppResult<i64> {
    let data_json = serde_json::to_string(data).unwrap();
    conn.execute(
        "INSERT INTO trading_import_rows (session_id, row_index, data) VALUES (?1, ?2, ?3)",
        params![session_id, row_index, data_json],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_import_rows_paginated(
    conn: &Connection,
    session_id: &str,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<TradingImportRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, row_index, data, status, error
         FROM trading_import_rows
         WHERE session_id = ?1
         ORDER BY row_index
         LIMIT ?2 OFFSET ?3",
    )?;

    let rows = stmt
        .query_map(params![session_id, limit, offset], |row| {
            let data_json: String = row.get(3)?;
            let data: ParsedTradingActivity =
                serde_json::from_str(&data_json).unwrap_or_else(|_| ParsedTradingActivity {
                    date: String::new(),
                    symbol: String::new(),
                    quantity: None,
                    activity_type: String::new(),
                    unit_price: None,
                    currency: "USD".to_string(),
                    fee: None,
                    row_number: 0,
                });

            Ok(TradingImportRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                row_index: row.get(2)?,
                data,
                status: row.get(4)?,
                error: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

pub fn count_import_rows(conn: &Connection, session_id: &str) -> AppResult<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM trading_import_rows WHERE session_id = ?1",
        params![session_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn get_pending_import_rows(
    conn: &Connection,
    session_id: &str,
) -> AppResult<Vec<TradingImportRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, row_index, data, status, error
         FROM trading_import_rows
         WHERE session_id = ?1 AND status = 'pending'
         ORDER BY row_index",
    )?;

    let rows = stmt
        .query_map(params![session_id], |row| {
            let data_json: String = row.get(3)?;
            let data: ParsedTradingActivity =
                serde_json::from_str(&data_json).unwrap_or_else(|_| ParsedTradingActivity {
                    date: String::new(),
                    symbol: String::new(),
                    quantity: None,
                    activity_type: String::new(),
                    unit_price: None,
                    currency: "USD".to_string(),
                    fee: None,
                    row_number: 0,
                });

            Ok(TradingImportRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                row_index: row.get(2)?,
                data,
                status: row.get(4)?,
                error: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

pub fn mark_import_row_imported(conn: &Connection, row_id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE trading_import_rows SET status = 'imported' WHERE id = ?1",
        params![row_id],
    )?;
    Ok(())
}

pub fn mark_import_row_error(conn: &Connection, row_id: i64, error: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE trading_import_rows SET status = 'error', error = ?2 WHERE id = ?1",
        params![row_id, error],
    )?;
    Ok(())
}

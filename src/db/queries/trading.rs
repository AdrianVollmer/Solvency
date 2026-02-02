use crate::error::AppResult;
use crate::models::trading::{
    ClosedPosition, NewTradingActivity, Position, TradingActivity, TradingActivityType,
    TradingImportRow, TradingImportSession, TradingImportStatus,
};
use crate::services::trading_csv_parser::ParsedTradingActivity;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use tracing::{debug, info};

struct ActivityRow {
    symbol: String,
    activity_type: String,
    quantity: Option<f64>,
    unit_price_cents: Option<i64>,
    _fee_cents: i64,
    currency: String,
}

struct ClosedPositionActivityRow {
    symbol: String,
    activity_type: String,
    quantity: Option<f64>,
    unit_price_cents: Option<i64>,
    currency: String,
    date: String,
}

struct PositionAccumulator {
    quantity: f64,
    total_cost: i64,
    total_proceeds: i64,
    total_fees: i64,
    total_taxes: i64,
    total_dividends: i64,
    currency: String,
    first_date: String,
    last_date: String,
}

fn trading_activity_from_row(row: &rusqlite::Row) -> rusqlite::Result<TradingActivity> {
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
        account_id: row.get(8)?,
        notes: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

// Activity operations

#[derive(Default)]
pub struct TradingActivityFilter {
    pub symbol: Option<String>,
    pub activity_type: Option<TradingActivityType>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// SQL ORDER BY expression (e.g., "date DESC"). Defaults to "date DESC, id DESC".
    pub sort_sql: Option<String>,
}

pub fn list_activities(
    conn: &Connection,
    filter: &TradingActivityFilter,
) -> rusqlite::Result<Vec<TradingActivity>> {
    let mut sql = String::from(
        "SELECT id, date, symbol, quantity, activity_type, unit_price_cents,
                currency, fee_cents, account_id, notes, created_at, updated_at
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

    // Use provided sort or default to date DESC
    let order_by = filter.sort_sql.as_deref().unwrap_or("date DESC");
    sql.push_str(&format!(" ORDER BY {}, id DESC", order_by));

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
        .query_map(params_refs.as_slice(), |row| trading_activity_from_row(row))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(activities)
}

/// Returns the earliest and latest trading activity dates, or `None` when the table is empty.
pub fn date_extent(conn: &Connection) -> rusqlite::Result<Option<(String, String)>> {
    conn.query_row(
        "SELECT MIN(date), MAX(date) FROM trading_activities",
        [],
        |row| {
            let min: Option<String> = row.get(0)?;
            let max: Option<String> = row.get(1)?;
            Ok(min.zip(max))
        },
    )
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
                currency, fee_cents, account_id, notes, created_at, updated_at
         FROM trading_activities WHERE id = ?",
        [id],
        trading_activity_from_row,
    )
    .optional()
}

pub fn create_activity(conn: &Connection, activity: &NewTradingActivity) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO trading_activities (date, symbol, quantity, activity_type, unit_price_cents, currency, fee_cents, account_id, notes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            activity.date,
            activity.symbol,
            activity.quantity,
            activity.activity_type.as_str(),
            activity.unit_price_cents,
            activity.currency,
            activity.fee_cents,
            activity.account_id,
            activity.notes,
        ],
    )?;
    let id = conn.last_insert_rowid();
    info!(
        activity_id = id,
        symbol = %activity.symbol,
        activity_type = %activity.activity_type.as_str(),
        "Created trading activity"
    );
    Ok(id)
}

pub fn update_activity(
    conn: &Connection,
    id: i64,
    activity: &NewTradingActivity,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE trading_activities SET date = ?, symbol = ?, quantity = ?, activity_type = ?,
         unit_price_cents = ?, currency = ?, fee_cents = ?, account_id = ?, notes = ?, updated_at = datetime('now')
         WHERE id = ?",
        params![
            activity.date,
            activity.symbol,
            activity.quantity,
            activity.activity_type.as_str(),
            activity.unit_price_cents,
            activity.currency,
            activity.fee_cents,
            activity.account_id,
            activity.notes,
            id,
        ],
    )?;
    info!(activity_id = id, symbol = %activity.symbol, "Updated trading activity");
    Ok(())
}

pub fn delete_activity(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    let rows = conn.execute("DELETE FROM trading_activities WHERE id = ?", [id])?;
    if rows > 0 {
        info!(activity_id = id, "Deleted trading activity");
    }
    Ok(rows > 0)
}

pub fn delete_all_activities(conn: &Connection) -> rusqlite::Result<usize> {
    let rows = conn.execute("DELETE FROM trading_activities", [])?;
    tracing::warn!(count = rows, "Deleted all trading activities");
    Ok(rows)
}

// Position calculations

/// Shared position calculation logic: takes raw activity rows and produces positions.
fn calculate_positions_from_activities(activities: Vec<ActivityRow>) -> Vec<Position> {
    let mut positions_map: HashMap<String, (f64, i64, String)> = HashMap::new();

    for row in activities {
        let activity_type: TradingActivityType = row
            .activity_type
            .parse()
            .unwrap_or(TradingActivityType::Buy);
        let qty = row.quantity.unwrap_or(0.0);
        let price = row.unit_price_cents.unwrap_or(0);

        let entry = positions_map
            .entry(row.symbol.clone())
            .or_insert((0.0, 0, row.currency));

        match activity_type {
            TradingActivityType::Buy => {
                let cost = (qty * price as f64).round() as i64;
                entry.0 += qty;
                entry.1 += cost;
            }
            TradingActivityType::Sell => {
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
            TradingActivityType::Split => {
                // Split adjustments are pre-applied to BUY/SELL quantities
                // when activities are created. No runtime adjustment needed.
            }
            TradingActivityType::Fee | TradingActivityType::Tax => {
                // These reduce cost basis (they're expenses associated with the position)
                entry.1 += (qty * price as f64).round() as i64;
            }
            TradingActivityType::Dividend => {
                // Dividends don't affect position quantity or cost basis
                // They're just income events
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

    // Sort alphabetically by symbol
    positions.sort_by(|a, b| a.symbol.cmp(&b.symbol));

    positions
}

pub fn get_positions(conn: &Connection) -> rusqlite::Result<Vec<Position>> {
    let mut stmt = conn.prepare(
        "SELECT symbol, activity_type, quantity, unit_price_cents, fee_cents, currency
         FROM trading_activities
         ORDER BY symbol, date ASC, id ASC",
    )?;

    let activities: Vec<ActivityRow> = stmt
        .query_map([], |row| {
            Ok(ActivityRow {
                symbol: row.get(0)?,
                activity_type: row.get(1)?,
                quantity: row.get(2)?,
                unit_price_cents: row.get(3)?,
                _fee_cents: row.get(4)?,
                currency: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(calculate_positions_from_activities(activities))
}

pub fn get_positions_for_account(
    conn: &Connection,
    account_id: i64,
) -> rusqlite::Result<Vec<Position>> {
    let mut stmt = conn.prepare(
        "SELECT symbol, activity_type, quantity, unit_price_cents, fee_cents, currency
         FROM trading_activities
         WHERE account_id = ?
         ORDER BY symbol, date ASC, id ASC",
    )?;

    let activities: Vec<ActivityRow> = stmt
        .query_map([account_id], |row| {
            Ok(ActivityRow {
                symbol: row.get(0)?,
                activity_type: row.get(1)?,
                quantity: row.get(2)?,
                unit_price_cents: row.get(3)?,
                _fee_cents: row.get(4)?,
                currency: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(calculate_positions_from_activities(activities))
}

pub fn get_positions_without_account(conn: &Connection) -> rusqlite::Result<Vec<Position>> {
    let mut stmt = conn.prepare(
        "SELECT symbol, activity_type, quantity, unit_price_cents, fee_cents, currency
         FROM trading_activities
         WHERE account_id IS NULL
         ORDER BY symbol, date ASC, id ASC",
    )?;

    let activities: Vec<ActivityRow> = stmt
        .query_map([], |row| {
            Ok(ActivityRow {
                symbol: row.get(0)?,
                activity_type: row.get(1)?,
                quantity: row.get(2)?,
                unit_price_cents: row.get(3)?,
                _fee_cents: row.get(4)?,
                currency: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(calculate_positions_from_activities(activities))
}

/// Get closed positions (where all securities have been sold)
pub fn get_closed_positions(conn: &Connection) -> rusqlite::Result<Vec<ClosedPosition>> {
    // Get all activities grouped by symbol
    let mut stmt = conn.prepare(
        "SELECT symbol, activity_type, quantity, unit_price_cents, currency, date
         FROM trading_activities
         ORDER BY symbol, date ASC, id ASC",
    )?;

    let activities: Vec<ClosedPositionActivityRow> = stmt
        .query_map([], |row| {
            Ok(ClosedPositionActivityRow {
                symbol: row.get(0)?,
                activity_type: row.get(1)?,
                quantity: row.get(2)?,
                unit_price_cents: row.get(3)?,
                currency: row.get(4)?,
                date: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Calculate positions by symbol, tracking cost, proceeds, fees, taxes, dividends, and dates
    let mut positions_map: HashMap<String, PositionAccumulator> = HashMap::new();

    for row in activities {
        let activity_type: TradingActivityType = row
            .activity_type
            .parse()
            .unwrap_or(TradingActivityType::Buy);
        let qty = row.quantity.unwrap_or(0.0);
        let price = row.unit_price_cents.unwrap_or(0);

        let entry = positions_map
            .entry(row.symbol.clone())
            .or_insert(PositionAccumulator {
                quantity: 0.0,
                total_cost: 0,
                total_proceeds: 0,
                total_fees: 0,
                total_taxes: 0,
                total_dividends: 0,
                currency: row.currency,
                first_date: row.date.clone(),
                last_date: row.date.clone(),
            });

        // Update last activity date
        if row.date > entry.last_date {
            entry.last_date = row.date.clone();
        }

        match activity_type {
            TradingActivityType::Buy => {
                let cost = (qty * price as f64).round() as i64;
                entry.quantity += qty;
                entry.total_cost += cost;
            }
            TradingActivityType::Sell => {
                let proceeds = (qty * price as f64).round() as i64;
                entry.quantity -= qty;
                entry.total_proceeds += proceeds;
                if entry.quantity < 0.0 {
                    entry.quantity = 0.0;
                }
            }
            TradingActivityType::Split => {
                // Split adjustments are pre-applied to BUY/SELL quantities
                // when activities are created. No runtime adjustment needed.
            }
            TradingActivityType::Fee => {
                // Fee amount is stored in unit_price_cents
                entry.total_fees += price;
            }
            TradingActivityType::Tax => {
                // Tax amount is stored in unit_price_cents
                entry.total_taxes += price;
            }
            TradingActivityType::Dividend => {
                // Dividend amount is stored in unit_price_cents
                entry.total_dividends += price;
            }
        }
    }

    // Convert to ClosedPosition structs, filtering to only zero positions
    let mut closed_positions: Vec<ClosedPosition> = positions_map
        .into_iter()
        .filter(|(_, acc)| acc.quantity == 0.0)
        .map(|(symbol, acc)| {
            // Net realized gain/loss = proceeds - cost + dividends - fees - taxes
            let realized_gain_loss_cents = acc.total_proceeds - acc.total_cost
                + acc.total_dividends
                - acc.total_fees
                - acc.total_taxes;
            ClosedPosition {
                symbol,
                total_cost_cents: acc.total_cost,
                total_proceeds_cents: acc.total_proceeds,
                realized_gain_loss_cents,
                currency: acc.currency,
                first_activity_date: acc.first_date,
                last_activity_date: acc.last_date,
            }
        })
        .collect();

    // Sort alphabetically by symbol
    closed_positions.sort_by(|a, b| a.symbol.cmp(&b.symbol));

    Ok(closed_positions)
}

/// Aggregate total fees and taxes across all trading activities.
/// Returns (total_fees_cents, total_taxes_cents).
pub fn get_portfolio_fee_tax_totals(conn: &Connection) -> rusqlite::Result<(i64, i64)> {
    conn.query_row(
        "SELECT
            COALESCE(SUM(CASE WHEN activity_type = 'FEE' THEN unit_price_cents ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN activity_type = 'TAX' THEN unit_price_cents ELSE 0 END), 0)
         FROM trading_activities",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
}

pub fn get_unique_symbols(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT DISTINCT symbol FROM trading_activities ORDER BY symbol")?;

    let symbols: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(symbols)
}

/// Get all activities for a specific symbol, ordered by date ascending (for XIRR calculation)
pub fn get_activities_for_symbol(
    conn: &Connection,
    symbol: &str,
) -> rusqlite::Result<Vec<TradingActivity>> {
    let mut stmt = conn.prepare(
        "SELECT id, date, symbol, quantity, activity_type, unit_price_cents,
                currency, fee_cents, account_id, notes, created_at, updated_at
         FROM trading_activities
         WHERE symbol = ?
         ORDER BY date ASC, id ASC",
    )?;

    let activities = stmt
        .query_map([symbol], trading_activity_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(activities)
}

/// Get the last BUY or SELL price for a symbol (for approximating current price)
/// Returns (price_cents, date) if found
pub fn get_last_trade_price(
    conn: &Connection,
    symbol: &str,
) -> rusqlite::Result<Option<(i64, String)>> {
    conn.query_row(
        "SELECT unit_price_cents, date
         FROM trading_activities
         WHERE symbol = ?
           AND activity_type IN ('BUY', 'SELL')
           AND unit_price_cents IS NOT NULL
         ORDER BY date DESC, id DESC
         LIMIT 1",
        [symbol],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
}

/// Get all BUY or SELL prices for a symbol in ascending date order.
/// Used to build a step function chart when no market data is available.
/// Returns Vec<(date, price_cents)>
pub fn get_all_trade_prices(
    conn: &Connection,
    symbol: &str,
) -> rusqlite::Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT date, unit_price_cents
         FROM trading_activities
         WHERE symbol = ?
           AND activity_type IN ('BUY', 'SELL')
           AND unit_price_cents IS NOT NULL
         ORDER BY date ASC, id ASC",
    )?;

    let rows = stmt.query_map([symbol], |row| Ok((row.get(0)?, row.get(1)?)))?;

    let mut prices = Vec::new();
    for row in rows {
        prices.push(row?);
    }
    Ok(prices)
}

// Import session operations

pub fn create_import_session(conn: &Connection, id: &str) -> AppResult<TradingImportSession> {
    conn.execute(
        "INSERT INTO trading_import_sessions (id, status) VALUES (?1, ?2)",
        params![id, TradingImportStatus::Parsing.as_str()],
    )?;
    info!(session_id = %id, "Created trading import session");
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
    info!(session_id = %id, status = %status.as_str(), "Updated trading import session status");
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
    debug!(session_id = %id, "Deleted trading import session");
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
                    account_id: None,
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
                    account_id: None,
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

// Split adjustment operations

/// Apply a split to all prior BUY/SELL activities for the same symbol.
/// Multiplies their quantity by the ratio and divides their unit_price by it,
/// recording original values in `trading_split_adjustments` for reversal.
pub fn apply_split_to_past_activities(
    conn: &Connection,
    split_activity_id: i64,
    symbol: &str,
    split_date: &str,
    ratio: f64,
) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, quantity, unit_price_cents
         FROM trading_activities
         WHERE symbol = ?1
           AND date < ?2
           AND activity_type IN ('BUY', 'SELL')
           AND quantity IS NOT NULL
           AND id NOT IN (
               SELECT target_activity_id FROM trading_split_adjustments
               WHERE split_activity_id = ?3
           )",
    )?;

    let targets: Vec<(i64, f64, Option<i64>)> = stmt
        .query_map(params![symbol, split_date, split_activity_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (target_id, original_qty, original_price) in &targets {
        conn.execute(
            "INSERT INTO trading_split_adjustments
             (split_activity_id, target_activity_id, original_quantity, original_unit_price_cents, split_ratio)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![split_activity_id, target_id, original_qty, original_price, ratio],
        )?;

        let new_qty = original_qty * ratio;
        let new_price = original_price.map(|p| (p as f64 / ratio).round() as i64);

        conn.execute(
            "UPDATE trading_activities
             SET quantity = ?1, unit_price_cents = ?2, updated_at = datetime('now')
             WHERE id = ?3",
            params![new_qty, new_price, target_id],
        )?;

        debug!(
            split_id = split_activity_id,
            target_id = target_id,
            original_qty = original_qty,
            new_qty = new_qty,
            "Applied split adjustment"
        );
    }

    if !targets.is_empty() {
        info!(
            symbol = %symbol,
            ratio = ratio,
            adjusted_count = targets.len(),
            "Applied split to past activities"
        );
    }

    Ok(())
}

/// Apply all existing splits (dated after this activity) to a newly created BUY/SELL.
pub fn apply_existing_splits_to_activity(
    conn: &Connection,
    activity_id: i64,
    symbol: &str,
    activity_date: &str,
) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, quantity
         FROM trading_activities
         WHERE symbol = ?1
           AND date > ?2
           AND activity_type = 'SPLIT'
           AND quantity IS NOT NULL
           AND quantity > 0
         ORDER BY date ASC, id ASC",
    )?;

    let splits: Vec<(i64, f64)> = stmt
        .query_map(params![symbol, activity_date], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if splits.is_empty() {
        return Ok(());
    }

    let (current_qty, current_price): (Option<f64>, Option<i64>) = conn.query_row(
        "SELECT quantity, unit_price_cents FROM trading_activities WHERE id = ?1",
        [activity_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let Some(mut running_qty) = current_qty else {
        return Ok(());
    };
    let mut running_price = current_price;

    for (split_id, ratio) in &splits {
        let already_applied: bool = conn.query_row(
            "SELECT COUNT(*) FROM trading_split_adjustments
             WHERE split_activity_id = ?1 AND target_activity_id = ?2",
            params![split_id, activity_id],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        )?;

        if already_applied {
            continue;
        }

        conn.execute(
            "INSERT INTO trading_split_adjustments
             (split_activity_id, target_activity_id, original_quantity, original_unit_price_cents, split_ratio)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![split_id, activity_id, running_qty, running_price, ratio],
        )?;

        running_qty *= ratio;
        running_price = running_price.map(|p| (p as f64 / ratio).round() as i64);
    }

    if Some(running_qty) != current_qty || running_price != current_price {
        conn.execute(
            "UPDATE trading_activities
             SET quantity = ?1, unit_price_cents = ?2, updated_at = datetime('now')
             WHERE id = ?3",
            params![running_qty, running_price, activity_id],
        )?;
    }

    Ok(())
}

/// Reverse all adjustments made by a specific split activity, restoring
/// target activities to the values they would have without this split.
///
/// Handles the case of multiple overlapping splits by recomputing from
/// the base (pre-any-split) values and re-applying remaining splits.
pub fn reverse_split_adjustments(
    conn: &Connection,
    split_activity_id: i64,
) -> rusqlite::Result<()> {
    // Collect the target activity ids affected by this split.
    let mut target_stmt = conn.prepare(
        "SELECT target_activity_id FROM trading_split_adjustments
         WHERE split_activity_id = ?1",
    )?;
    let target_ids: Vec<i64> = target_stmt
        .query_map([split_activity_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(target_stmt);

    let target_count = target_ids.len();
    for target_id in target_ids {
        recompute_target_without_split(conn, target_id, split_activity_id)?;
    }

    // Delete the adjustment records for this split.
    conn.execute(
        "DELETE FROM trading_split_adjustments WHERE split_activity_id = ?1",
        [split_activity_id],
    )?;

    info!(
        split_activity_id = split_activity_id,
        reversed_count = target_count,
        "Reversed split adjustments"
    );

    Ok(())
}

/// Remove adjustment records that target a specific activity (for cleanup
/// when a BUY/SELL is deleted—no reversal needed since the activity is going away).
pub fn delete_adjustments_targeting_activity(
    conn: &Connection,
    target_activity_id: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM trading_split_adjustments WHERE target_activity_id = ?1",
        [target_activity_id],
    )?;
    Ok(())
}

/// Recompute a target activity's quantity/price after removing one split.
///
/// Algorithm:
/// 1. Find the base value (original before any split) from the earliest adjustment.
/// 2. Delete the adjustment record for the removed split.
/// 3. Re-apply remaining splits in chronological order, updating their
///    stored originals along the way.
/// 4. Write the final value back to the activity.
fn recompute_target_without_split(
    conn: &Connection,
    target_id: i64,
    removed_split_id: i64,
) -> rusqlite::Result<()> {
    // Get the base (pre-any-split) values: the original from the earliest adjustment.
    let base: Option<(f64, Option<i64>)> = conn
        .query_row(
            "SELECT sa.original_quantity, sa.original_unit_price_cents
             FROM trading_split_adjustments sa
             JOIN trading_activities s ON s.id = sa.split_activity_id
             WHERE sa.target_activity_id = ?1
             ORDER BY s.date ASC, s.id ASC
             LIMIT 1",
            [target_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let Some((base_qty, base_price)) = base else {
        return Ok(());
    };

    // Delete the record for the removed split.
    conn.execute(
        "DELETE FROM trading_split_adjustments
         WHERE split_activity_id = ?1 AND target_activity_id = ?2",
        params![removed_split_id, target_id],
    )?;

    // Collect remaining adjustments in chronological order.
    let mut remaining_stmt = conn.prepare(
        "SELECT sa.id, sa.split_ratio
         FROM trading_split_adjustments sa
         JOIN trading_activities s ON s.id = sa.split_activity_id
         WHERE sa.target_activity_id = ?1
         ORDER BY s.date ASC, s.id ASC",
    )?;
    let remaining: Vec<(i64, f64)> = remaining_stmt
        .query_map([target_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    // Replay remaining adjustments from the base values.
    let mut running_qty = base_qty;
    let mut running_price = base_price;

    for (adj_id, ratio) in &remaining {
        conn.execute(
            "UPDATE trading_split_adjustments
             SET original_quantity = ?1, original_unit_price_cents = ?2
             WHERE id = ?3",
            params![running_qty, running_price, adj_id],
        )?;

        running_qty *= ratio;
        running_price = running_price.map(|p| (p as f64 / ratio).round() as i64);
    }

    // Write the final computed values to the activity.
    conn.execute(
        "UPDATE trading_activities
         SET quantity = ?1, unit_price_cents = ?2, updated_at = datetime('now')
         WHERE id = ?3",
        params![running_qty, running_price, target_id],
    )?;

    debug!(
        target_id = target_id,
        removed_split = removed_split_id,
        final_qty = running_qty,
        "Recomputed target after split removal"
    );

    Ok(())
}

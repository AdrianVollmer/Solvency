use crate::models::market_data::{MarketData, NewMarketData, SymbolDataCoverage};
use rusqlite::{params, Connection, OptionalExtension};

/// Maximum gap in days that's considered acceptable (weekends + holidays)
pub const MAX_GAP_DAYS: i64 = 5;

/// Insert or update market data for a symbol on a date
pub fn upsert_market_data(conn: &Connection, data: &NewMarketData) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO market_data (symbol, date, close_price_cents, currency)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(symbol, date) DO UPDATE SET
         close_price_cents = excluded.close_price_cents,
         currency = excluded.currency,
         fetched_at = datetime('now')",
        params![
            data.symbol,
            data.date,
            data.close_price_cents,
            data.currency
        ],
    )?;
    Ok(())
}

/// Insert multiple market data points
pub fn insert_market_data_batch(conn: &Connection, data: &[NewMarketData]) -> rusqlite::Result<()> {
    for item in data {
        upsert_market_data(conn, item)?;
    }
    Ok(())
}

/// Get the latest market data for a symbol
pub fn get_latest_price(conn: &Connection, symbol: &str) -> rusqlite::Result<Option<MarketData>> {
    conn.query_row(
        "SELECT id, symbol, date, close_price_cents, currency, fetched_at
         FROM market_data
         WHERE symbol = ?1
         ORDER BY date DESC
         LIMIT 1",
        [symbol],
        |row| {
            Ok(MarketData {
                id: row.get(0)?,
                symbol: row.get(1)?,
                date: row.get(2)?,
                close_price_cents: row.get(3)?,
                currency: row.get(4)?,
                fetched_at: row.get(5)?,
            })
        },
    )
    .optional()
}

/// Get market data for a symbol on a specific date
pub fn get_price_for_date(
    conn: &Connection,
    symbol: &str,
    date: &str,
) -> rusqlite::Result<Option<MarketData>> {
    conn.query_row(
        "SELECT id, symbol, date, close_price_cents, currency, fetched_at
         FROM market_data
         WHERE symbol = ?1 AND date = ?2",
        [symbol, date],
        |row| {
            Ok(MarketData {
                id: row.get(0)?,
                symbol: row.get(1)?,
                date: row.get(2)?,
                close_price_cents: row.get(3)?,
                currency: row.get(4)?,
                fetched_at: row.get(5)?,
            })
        },
    )
    .optional()
}

/// Get all market data for a symbol
pub fn get_prices_for_symbol(conn: &Connection, symbol: &str) -> rusqlite::Result<Vec<MarketData>> {
    let mut stmt = conn.prepare(
        "SELECT id, symbol, date, close_price_cents, currency, fetched_at
         FROM market_data
         WHERE symbol = ?1
         ORDER BY date DESC",
    )?;

    let data = stmt
        .query_map([symbol], |row| {
            Ok(MarketData {
                id: row.get(0)?,
                symbol: row.get(1)?,
                date: row.get(2)?,
                close_price_cents: row.get(3)?,
                currency: row.get(4)?,
                fetched_at: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(data)
}

/// Get data coverage summary for all symbols that have positions (both open and closed)
pub fn get_symbol_coverage(conn: &Connection) -> rusqlite::Result<Vec<SymbolDataCoverage>> {
    // Get symbols with their activity date ranges (all non-cash symbols)
    let mut stmt = conn.prepare(
        "WITH position_symbols AS (
            SELECT symbol, currency,
                   MIN(date) as first_activity_date,
                   MAX(date) as last_activity_date,
                   SUM(CASE
                       WHEN activity_type IN ('BUY', 'ADD_HOLDING', 'TRANSFER_IN') THEN COALESCE(quantity, 0)
                       WHEN activity_type IN ('SELL', 'REMOVE_HOLDING', 'TRANSFER_OUT') THEN -COALESCE(quantity, 0)
                       ELSE 0
                   END) as net_quantity
            FROM trading_activities
            WHERE symbol NOT LIKE '$CASH-%'
            GROUP BY symbol
        ),
        market_data_summary AS (
            SELECT symbol,
                   MIN(date) as first_data_date,
                   MAX(date) as last_data_date,
                   COUNT(*) as data_points
            FROM market_data
            GROUP BY symbol
        )
        SELECT
            ps.symbol,
            ps.currency,
            ps.first_activity_date,
            ps.last_activity_date,
            mds.first_data_date,
            mds.last_data_date,
            COALESCE(mds.data_points, 0) as data_points,
            ps.net_quantity
        FROM position_symbols ps
        LEFT JOIN market_data_summary mds ON ps.symbol = mds.symbol
        ORDER BY ps.net_quantity > 0 DESC, ps.symbol",
    )?;

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let coverage = stmt
        .query_map([], |row| {
            let last_data_date: Option<String> = row.get(5)?;
            let last_activity_date: String = row.get(3)?;
            let net_quantity: f64 = row.get(7)?;
            let is_closed = net_quantity <= 0.0;

            // For closed positions, check if data covers up to last_activity_date
            // For open positions, check if data covers up to today
            let target_date = if is_closed {
                &last_activity_date
            } else {
                &today
            };

            let has_current_price = last_data_date
                .as_ref()
                .map(|d| {
                    d >= target_date || {
                        // Check if within acceptable gap (for weekends/holidays)
                        if let (Ok(last), Ok(target)) = (
                            chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d"),
                            chrono::NaiveDate::parse_from_str(target_date, "%Y-%m-%d"),
                        ) {
                            (target - last).num_days() <= MAX_GAP_DAYS
                        } else {
                            false
                        }
                    }
                })
                .unwrap_or(false);

            // Calculate missing days (rough estimate)
            let first_activity: String = row.get(2)?;
            let data_points: i64 = row.get(6)?;
            let missing_days =
                if let Ok(first) = chrono::NaiveDate::parse_from_str(&first_activity, "%Y-%m-%d") {
                    if let Ok(end) = chrono::NaiveDate::parse_from_str(target_date, "%Y-%m-%d") {
                        // Approximate trading days (weekdays only, ~252 per year)
                        let total_days = (end - first).num_days();
                        let approx_trading_days = (total_days as f64 * 5.0 / 7.0) as i64;
                        (approx_trading_days - data_points).max(0)
                    } else {
                        0
                    }
                } else {
                    0
                };

            Ok(SymbolDataCoverage {
                symbol: row.get(0)?,
                currency: row.get(1)?,
                first_activity_date: row.get(2)?,
                last_activity_date,
                first_data_date: row.get(4)?,
                last_data_date,
                data_points,
                missing_days,
                has_current_price,
                is_closed,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(coverage)
}

/// Get symbols that need market data updates
/// Includes both open positions (end_date = today) and closed positions (end_date = last_activity_date)
pub fn get_symbols_needing_data(
    conn: &Connection,
) -> rusqlite::Result<Vec<(String, String, String)>> {
    // Returns (symbol, start_date, end_date) for symbols that need data
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let mut stmt = conn.prepare(
        "WITH all_traded_symbols AS (
            SELECT symbol,
                   MIN(date) as first_activity_date,
                   MAX(date) as last_activity_date,
                   SUM(CASE
                       WHEN activity_type IN ('BUY', 'ADD_HOLDING', 'TRANSFER_IN') THEN COALESCE(quantity, 0)
                       WHEN activity_type IN ('SELL', 'REMOVE_HOLDING', 'TRANSFER_OUT') THEN -COALESCE(quantity, 0)
                       ELSE 0
                   END) as net_quantity
            FROM trading_activities
            WHERE symbol NOT LIKE '$CASH-%'
            GROUP BY symbol
        ),
        latest_data AS (
            SELECT symbol, MAX(date) as last_data_date
            FROM market_data
            GROUP BY symbol
        )
        SELECT
            ats.symbol,
            COALESCE(ld.last_data_date, ats.first_activity_date) as start_date,
            CASE
                WHEN ats.net_quantity > 0 THEN ?1
                ELSE ats.last_activity_date
            END as end_date,
            ats.net_quantity
        FROM all_traded_symbols ats
        LEFT JOIN latest_data ld ON ats.symbol = ld.symbol
        WHERE ld.last_data_date IS NULL
           OR (ats.net_quantity > 0 AND ld.last_data_date < date(?1, '-' || ?2 || ' days'))
           OR (ats.net_quantity <= 0 AND ld.last_data_date < ats.last_activity_date)
        ORDER BY ats.symbol",
    )?;

    let symbols = stmt
        .query_map(rusqlite::params![&today, MAX_GAP_DAYS], |row| {
            let symbol: String = row.get(0)?;
            let start_date: String = row.get(1)?;
            let end_date: String = row.get(2)?;
            Ok((symbol, start_date, end_date))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(symbols)
}

/// Delete all market data for a symbol
pub fn delete_market_data_for_symbol(conn: &Connection, symbol: &str) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM market_data WHERE symbol = ?1", [symbol])
}

/// Get the count of market data points
pub fn count_market_data(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM market_data", [], |row| row.get(0))
}

/// Get all unique symbols in market data
pub fn get_symbols_with_data(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT symbol FROM market_data ORDER BY symbol")?;

    let symbols = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(symbols)
}

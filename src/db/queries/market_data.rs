use crate::models::market_data::{MarketData, NewMarketData, SymbolDataCoverage};
use rusqlite::{params, Connection, OptionalExtension};

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

/// Get data coverage summary for all symbols that have positions
pub fn get_symbol_coverage(conn: &Connection) -> rusqlite::Result<Vec<SymbolDataCoverage>> {
    // Get symbols with their activity date ranges (only non-cash symbols with current positions > 0)
    let mut stmt = conn.prepare(
        "WITH position_symbols AS (
            SELECT symbol, currency,
                   MIN(date) as first_activity_date,
                   MAX(date) as last_activity_date
            FROM trading_activities
            WHERE symbol NOT LIKE '$CASH-%'
            GROUP BY symbol
            HAVING SUM(CASE
                WHEN activity_type IN ('BUY', 'ADD_HOLDING', 'TRANSFER_IN') THEN COALESCE(quantity, 0)
                WHEN activity_type IN ('SELL', 'REMOVE_HOLDING', 'TRANSFER_OUT') THEN -COALESCE(quantity, 0)
                ELSE 0
            END) > 0
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
            COALESCE(mds.data_points, 0) as data_points
        FROM position_symbols ps
        LEFT JOIN market_data_summary mds ON ps.symbol = mds.symbol
        ORDER BY ps.symbol",
    )?;

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let coverage = stmt
        .query_map([], |row| {
            let last_data_date: Option<String> = row.get(5)?;
            let has_current_price = last_data_date
                .as_ref()
                .map(|d| {
                    d >= &today || {
                        // Check if within last 3 days (for weekends/holidays)
                        if let (Ok(last), Ok(now)) = (
                            chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d"),
                            chrono::NaiveDate::parse_from_str(&today, "%Y-%m-%d"),
                        ) {
                            (now - last).num_days() <= 3
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
                    if let Ok(now) = chrono::NaiveDate::parse_from_str(&today, "%Y-%m-%d") {
                        // Approximate trading days (weekdays only, ~252 per year)
                        let total_days = (now - first).num_days();
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
                last_activity_date: row.get(3)?,
                first_data_date: row.get(4)?,
                last_data_date,
                data_points,
                missing_days,
                has_current_price,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(coverage)
}

/// Get symbols that need market data updates
pub fn get_symbols_needing_data(
    conn: &Connection,
) -> rusqlite::Result<Vec<(String, String, String)>> {
    // Returns (symbol, start_date, end_date) for symbols that need data
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let mut stmt = conn.prepare(
        "WITH position_symbols AS (
            SELECT symbol,
                   MIN(date) as first_activity_date
            FROM trading_activities
            WHERE symbol NOT LIKE '$CASH-%'
            GROUP BY symbol
            HAVING SUM(CASE
                WHEN activity_type IN ('BUY', 'ADD_HOLDING', 'TRANSFER_IN') THEN COALESCE(quantity, 0)
                WHEN activity_type IN ('SELL', 'REMOVE_HOLDING', 'TRANSFER_OUT') THEN -COALESCE(quantity, 0)
                ELSE 0
            END) > 0
        ),
        latest_data AS (
            SELECT symbol, MAX(date) as last_data_date
            FROM market_data
            GROUP BY symbol
        )
        SELECT
            ps.symbol,
            COALESCE(ld.last_data_date, ps.first_activity_date) as start_date
        FROM position_symbols ps
        LEFT JOIN latest_data ld ON ps.symbol = ld.symbol
        WHERE ld.last_data_date IS NULL
           OR ld.last_data_date < ?1
        ORDER BY ps.symbol",
    )?;

    let symbols = stmt
        .query_map([&today], |row| {
            let symbol: String = row.get(0)?;
            let start_date: String = row.get(1)?;
            Ok((symbol, start_date, today.clone()))
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

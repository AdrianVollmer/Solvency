use rusqlite::Connection;

/// Daily transaction sum: (date, amount_cents)
pub type DailyTransactionSum = (String, i64);

/// Activity row for net worth: (date, symbol, activity_type, quantity, unit_price_cents, fee_cents, currency)
pub type NetWorthActivityRow = (
    String,
    String,
    String,
    Option<f64>,
    Option<i64>,
    i64,
    String,
);

/// Market data row: (symbol, date, close_price_cents)
pub type MarketDataRow = (String, String, i64);

/// Last trade price row: (symbol, price_cents, date)
pub type LastTradePriceRow = (String, i64, String);

/// Get daily transaction sums (grouped by date, ordered ascending)
pub fn get_daily_transaction_sums(conn: &Connection) -> rusqlite::Result<Vec<DailyTransactionSum>> {
    let mut stmt = conn.prepare(
        "SELECT date, SUM(amount_cents) as daily_sum
         FROM transactions
         GROUP BY date
         ORDER BY date ASC",
    )?;

    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Get all trading activities ordered by date (for chronological processing)
pub fn get_all_activities_ordered(conn: &Connection) -> rusqlite::Result<Vec<NetWorthActivityRow>> {
    let mut stmt = conn.prepare(
        "SELECT date, symbol, activity_type, quantity, unit_price_cents, fee_cents, currency
         FROM trading_activities
         ORDER BY date ASC, id ASC",
    )?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Get all market data for price lookups
pub fn get_all_market_data(conn: &Connection) -> rusqlite::Result<Vec<MarketDataRow>> {
    let mut stmt = conn.prepare(
        "SELECT symbol, date, close_price_cents
         FROM market_data
         ORDER BY symbol, date ASC",
    )?;

    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Get last trade price for each symbol (for fallback pricing)
pub fn get_last_trade_prices(conn: &Connection) -> rusqlite::Result<Vec<LastTradePriceRow>> {
    let mut stmt = conn.prepare(
        "SELECT t.symbol, t.unit_price_cents, t.date
         FROM trading_activities t
         INNER JOIN (
             SELECT symbol, MAX(date || '-' || printf('%010d', id)) as max_key
             FROM trading_activities
             WHERE activity_type IN ('BUY', 'SELL')
               AND unit_price_cents IS NOT NULL
             GROUP BY symbol
         ) latest ON t.symbol = latest.symbol
             AND (t.date || '-' || printf('%010d', t.id)) = latest.max_key
         WHERE t.activity_type IN ('BUY', 'SELL')
           AND t.unit_price_cents IS NOT NULL",
    )?;

    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Get the earliest date from transactions or trading activities
pub fn get_earliest_date(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT MIN(date) FROM (
            SELECT MIN(date) as date FROM transactions
            UNION ALL
            SELECT MIN(date) as date FROM trading_activities
        )",
        [],
        |row| row.get(0),
    )
}

/// Get the latest date from transactions or trading activities
pub fn get_latest_date(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT MAX(date) FROM (
            SELECT MAX(date) as date FROM transactions
            UNION ALL
            SELECT MAX(date) as date FROM trading_activities
        )",
        [],
        |row| row.get(0),
    )
}

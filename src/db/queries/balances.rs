use rusqlite::Connection;
use std::collections::HashMap;

/// Returns a map of account_id -> sum of amount_cents for all transactions
/// that have an account_id set.
pub fn get_cash_account_balances(conn: &Connection) -> rusqlite::Result<HashMap<i64, i64>> {
    let mut stmt = conn.prepare(
        "SELECT account_id, COALESCE(SUM(amount_cents), 0)
         FROM transactions
         WHERE account_id IS NOT NULL
         GROUP BY account_id",
    )?;

    let rows: Vec<(i64, i64)> = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut map = HashMap::new();
    for (account_id, balance) in rows {
        map.insert(account_id, balance);
    }
    Ok(map)
}

/// Returns the sum of amount_cents for all transactions without an account.
pub fn get_unassociated_cash_balance(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM transactions WHERE account_id IS NULL",
        [],
        |row| row.get(0),
    )
}

use crate::models::account::{Account, AccountType, NewAccount};
use rusqlite::{params, Connection, OptionalExtension};
use tracing::{info, warn};

pub fn list_accounts(conn: &Connection) -> rusqlite::Result<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, account_type, created_at, updated_at
         FROM accounts
         ORDER BY name",
    )?;

    let accounts = stmt
        .query_map([], |row| {
            let account_type_str: String = row.get(2)?;
            Ok(Account {
                id: row.get(0)?,
                name: row.get(1)?,
                account_type: AccountType::parse(&account_type_str).unwrap_or(AccountType::Cash),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?
        .filter_map(|a| a.ok())
        .collect();

    Ok(accounts)
}

pub fn list_accounts_by_type(
    conn: &Connection,
    account_type: AccountType,
) -> rusqlite::Result<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, account_type, created_at, updated_at
         FROM accounts
         WHERE account_type = ?
         ORDER BY name",
    )?;

    let accounts = stmt
        .query_map([account_type.as_str()], |row| {
            let account_type_str: String = row.get(2)?;
            Ok(Account {
                id: row.get(0)?,
                name: row.get(1)?,
                account_type: AccountType::parse(&account_type_str).unwrap_or(AccountType::Cash),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?
        .filter_map(|a| a.ok())
        .collect();

    Ok(accounts)
}

pub fn get_account(conn: &Connection, id: i64) -> rusqlite::Result<Option<Account>> {
    conn.query_row(
        "SELECT id, name, account_type, created_at, updated_at FROM accounts WHERE id = ?",
        [id],
        |row| {
            let account_type_str: String = row.get(2)?;
            Ok(Account {
                id: row.get(0)?,
                name: row.get(1)?,
                account_type: AccountType::parse(&account_type_str).unwrap_or(AccountType::Cash),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )
    .optional()
}

pub fn create_account(conn: &Connection, account: &NewAccount) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO accounts (name, account_type) VALUES (?, ?)",
        params![account.name, account.account_type.as_str()],
    )?;
    let id = conn.last_insert_rowid();
    info!(account_id = id, name = %account.name, "Created account");
    Ok(id)
}

pub fn update_account(conn: &Connection, id: i64, account: &NewAccount) -> rusqlite::Result<bool> {
    let rows = conn.execute(
        "UPDATE accounts SET name = ?, account_type = ?, updated_at = datetime('now') WHERE id = ?",
        params![account.name, account.account_type.as_str(), id],
    )?;
    if rows > 0 {
        info!(account_id = id, name = %account.name, "Updated account");
    }
    Ok(rows > 0)
}

pub fn delete_account(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    let rows = conn.execute("DELETE FROM accounts WHERE id = ?", [id])?;
    if rows > 0 {
        info!(account_id = id, "Deleted account");
    }
    Ok(rows > 0)
}

pub fn delete_all_accounts(conn: &Connection) -> rusqlite::Result<usize> {
    let rows = conn.execute("DELETE FROM accounts", [])?;
    warn!(count = rows, "Deleted all accounts");
    Ok(rows)
}

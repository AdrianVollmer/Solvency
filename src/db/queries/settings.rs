use rusqlite::{params, Connection};
use std::collections::HashMap;

use crate::error::AppResult;
use crate::models::Settings;

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    match conn.query_row("SELECT value FROM settings WHERE key = ?", [key], |row| {
        row.get(0)
    }) {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn get_all_settings(conn: &Connection) -> rusqlite::Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;

    let settings = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<HashMap<_, _>, _>>()?;

    Ok(settings)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value, updated_at)
         VALUES (?, ?, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value],
    )?;
    Ok(())
}

pub fn delete_setting(conn: &Connection, key: &str) -> rusqlite::Result<bool> {
    let rows = conn.execute("DELETE FROM settings WHERE key = ?", [key])?;
    Ok(rows > 0)
}

/// Fetch all settings and convert to Settings struct.
/// This is a convenience function that combines get_all_settings and Settings::from_map.
pub fn get_settings(conn: &Connection) -> AppResult<Settings> {
    let settings_map = get_all_settings(conn)?;
    Ok(Settings::from_map(settings_map))
}

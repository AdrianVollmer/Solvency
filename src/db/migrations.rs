use rusqlite::Connection;
use std::fs;
use std::path::Path;

pub fn run_migrations(conn: &Connection, migrations_dir: &Path) -> rusqlite::Result<()> {
    tracing::debug!(dir = %migrations_dir.display(), "Checking for database migrations");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS _migrations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    let mut entries: Vec<_> = fs::read_dir(migrations_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext == "sql")
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();

    entries.sort_by_key(|e| e.file_name());
    tracing::debug!(count = entries.len(), "Found migration files");

    let mut applied_count = 0;
    for entry in entries {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        let already_applied: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = ?)",
            [&*name],
            |row| row.get(0),
        )?;

        if !already_applied {
            let sql = fs::read_to_string(entry.path())
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            tracing::info!(migration = %name, "Applying migration");
            conn.execute_batch(&sql)?;

            conn.execute("INSERT INTO _migrations (name) VALUES (?)", [&*name])?;
            applied_count += 1;
        }
    }

    if applied_count > 0 {
        tracing::info!(count = applied_count, "Migrations applied successfully");
    } else {
        tracing::debug!("No new migrations to apply");
    }

    Ok(())
}

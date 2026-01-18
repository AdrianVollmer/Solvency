use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;

pub type DbPool = Pool<SqliteConnectionManager>;

pub fn create_pool(database_path: &Path) -> Result<DbPool, r2d2::Error> {
    tracing::info!(path = %database_path.display(), "Creating database connection pool");

    if let Some(parent) = database_path.parent() {
        if std::fs::create_dir_all(parent).is_ok() {
            tracing::debug!(dir = %parent.display(), "Ensured database directory exists");
        }
    }

    let manager = SqliteConnectionManager::file(database_path).with_init(|conn| {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA foreign_keys = ON;
                 PRAGMA busy_timeout = 5000;",
        )
    });

    let pool = Pool::builder().max_size(10).build(manager)?;
    tracing::info!(max_size = 10, "Database connection pool created");
    Ok(pool)
}

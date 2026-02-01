use askama::Template;
use axum::extract::{Multipart, State};
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::Form;
use serde::Deserialize;
use std::fs;
use std::path::Path;

use tracing::{info, warn};

use crate::db::queries::settings;
use crate::error::{AppError, AppResult, RenderHtml};
use crate::models::Settings;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/settings.html")]
pub struct SettingsTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub database_size: String,
}

#[derive(Template)]
#[template(path = "partials/settings_saved.html")]
pub struct SettingsSavedTemplate {
    pub icons: crate::filters::Icons,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct SettingsFormData {
    pub theme: String,
    pub currency: String,
    pub date_format: String,
    pub page_size: String,
    pub locale: String,
}

#[derive(Debug, Deserialize)]
pub struct ThemeFormData {
    pub theme: String,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let app_settings = state.load_settings()?;

    let database_size = get_database_size(&state.config.database_path);

    let template = SettingsTemplate {
        title: "Settings".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        database_size,
    };

    template.render_html()
}

fn get_database_size(path: &std::path::Path) -> String {
    match fs::metadata(path) {
        Ok(metadata) => {
            let size = metadata.len();
            format_size(size)
        }
        Err(_) => "Unknown".to_string(),
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

pub async fn update(
    State(state): State<AppState>,
    Form(form): Form<SettingsFormData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    settings::set_setting(&conn, "theme", &form.theme)?;
    settings::set_setting(&conn, "currency", &form.currency)?;
    settings::set_setting(&conn, "date_format", &form.date_format)?;
    settings::set_setting(&conn, "page_size", &form.page_size)?;
    settings::set_setting(&conn, "locale", &form.locale)?;

    let template = SettingsSavedTemplate {
        icons: crate::filters::Icons,
        message: "Settings saved successfully".into(),
    };

    template.render_html()
}

pub async fn toggle_theme(
    State(state): State<AppState>,
    Form(form): Form<ThemeFormData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    settings::set_setting(&conn, "theme", &form.theme)?;

    Ok(Html(String::new()))
}

pub async fn export_database(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let conn = state.db.get()?;

    let temp_path =
        std::env::temp_dir().join(format!("solvency-backup-{}.db", std::process::id()));
    let path_str = temp_path.display().to_string().replace('\'', "''");

    // VACUUM INTO creates an atomic, consistent snapshot of the database.
    conn.execute_batch(&format!("VACUUM INTO '{}'", path_str))?;

    let bytes = fs::read(&temp_path)?;
    let _ = fs::remove_file(&temp_path);

    info!(size_bytes = bytes.len(), "Database exported");

    Ok((
        [
            (header::CONTENT_TYPE, "application/x-sqlite3"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"solvency-backup.db\"",
            ),
        ],
        bytes,
    ))
}

pub async fn import_database(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<Html<String>> {
    let mut file_bytes = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read upload: {}", e)))?
    {
        if field.name() == Some("file") {
            file_bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::Internal(format!("Failed to read file: {}", e)))?
                .to_vec();
            break;
        }
    }

    if file_bytes.is_empty() {
        return Err(AppError::Validation("No file uploaded".to_string()));
    }

    let mut conn = state.db.get()?;

    // Only accept SQLite binary files
    let is_sqlite = file_bytes.len() >= 16 && file_bytes[..16] == *b"SQLite format 3\0";

    if !is_sqlite {
        return Err(AppError::Validation(
            "Invalid file format. Please upload a .db SQLite backup file.".to_string(),
        ));
    }

    let temp_path =
        std::env::temp_dir().join(format!("solvency-import-{}.db", std::process::id()));
    fs::write(&temp_path, &file_bytes)?;

    let result = restore_from_db_file(&mut conn, &temp_path, &state.config.migrations_path);
    let _ = fs::remove_file(&temp_path);
    result?;

    info!(
        size_bytes = file_bytes.len(),
        "Database restored from .db backup"
    );

    state.cache.invalidate();

    let template = SettingsSavedTemplate {
        icons: crate::filters::Icons,
        message: "Database imported successfully. Please refresh the page.".into(),
    };

    template.render_html()
}

/// Restore the live database from an uploaded .db file using SQLite's backup API.
///
/// This performs a page-level copy from the source into the live database.
/// All existing pool connections see the new data immediately. After the
/// backup completes, migrations are re-run to bring the schema up to date
/// (handles importing backups from older versions).
fn restore_from_db_file(
    conn: &mut rusqlite::Connection,
    src_path: &Path,
    migrations_path: &Path,
) -> AppResult<()> {
    let src = rusqlite::Connection::open(src_path)?;
    let backup = rusqlite::backup::Backup::new(&src, conn)?;
    backup
        .run_to_completion(100, std::time::Duration::ZERO, None)
        .map_err(|e| AppError::Internal(format!("Backup failed: {}", e)))?;
    drop(backup);
    drop(src);

    // Re-run migrations so the schema matches the current app version
    crate::db::migrations::run_migrations(conn, migrations_path)?;

    // Restore WAL mode and FK checks (backup copies source pragmas)
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;\
         PRAGMA foreign_keys = ON;",
    )?;

    Ok(())
}

pub async fn clear_database(State(state): State<AppState>) -> AppResult<Html<String>> {
    warn!("Clearing entire database");
    let conn = state.db.get()?;

    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master
         WHERE type='table'
         AND name NOT LIKE 'sqlite_%'
         AND name != '_migrations'
         ORDER BY name",
    )?;

    let tables: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    for table in &tables {
        conn.execute(&format!("DELETE FROM \"{}\"", table), [])?;
    }

    warn!(tables_cleared = tables.len(), "Database cleared");

    Ok(Html(String::new()))
}

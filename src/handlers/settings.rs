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

    let conn = state.db.get()?;

    // Detect format: SQLite binary (.db) vs SQL text (.sql)
    let is_sqlite = file_bytes.len() >= 16 && file_bytes[..16] == *b"SQLite format 3\0";

    if is_sqlite {
        let temp_path =
            std::env::temp_dir().join(format!("solvency-import-{}.db", std::process::id()));
        fs::write(&temp_path, &file_bytes)?;

        let result = restore_from_db_file(&conn, &temp_path);
        let _ = fs::remove_file(&temp_path);
        result?;

        info!(
            size_bytes = file_bytes.len(),
            "Database restored from .db backup"
        );
    } else {
        // Legacy SQL import
        let sql_content = String::from_utf8(file_bytes)
            .map_err(|e| AppError::Validation(format!("Invalid file: {}", e)))?;
        conn.execute_batch("PRAGMA foreign_keys = OFF")?;
        let result = conn.execute_batch(&sql_content);
        let _ = conn.execute_batch("PRAGMA foreign_keys = ON");
        result?;

        info!(
            size_bytes = sql_content.len(),
            "Database imported from SQL file"
        );
    }

    state.cache.invalidate();

    let template = SettingsSavedTemplate {
        icons: crate::filters::Icons,
        message: "Database imported successfully. Please refresh the page.".into(),
    };

    template.render_html()
}

/// Restore the live database from an uploaded .db file using ATTACH + copy.
fn restore_from_db_file(conn: &rusqlite::Connection, src_path: &Path) -> AppResult<()> {
    let path_str = src_path.display().to_string().replace('\'', "''");
    conn.execute_batch(&format!("ATTACH DATABASE '{}' AS src", path_str))?;

    let result = (|| -> AppResult<()> {
        // Read source schema
        let mut stmt = conn.prepare(
            "SELECT name, sql FROM src.sqlite_master \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND sql IS NOT NULL",
        )?;
        let tables: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        drop(stmt);

        let mut stmt = conn.prepare(
            "SELECT sql FROM src.sqlite_master \
             WHERE type IN ('index', 'trigger') \
             AND name NOT LIKE 'sqlite_%' AND sql IS NOT NULL",
        )?;
        let extra_objects: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        drop(stmt);

        // Get existing tables to drop
        let mut stmt = conn.prepare(
            "SELECT name FROM main.sqlite_master \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )?;
        let existing: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        drop(stmt);

        // Disable FK checks (must be outside transaction)
        conn.execute_batch("PRAGMA foreign_keys = OFF")?;

        conn.execute_batch("BEGIN")?;

        for name in &existing {
            conn.execute_batch(&format!("DROP TABLE IF EXISTS main.\"{}\"", name))?;
        }
        for (_, create_sql) in &tables {
            conn.execute_batch(create_sql)?;
        }
        for (name, _) in &tables {
            conn.execute_batch(&format!(
                "INSERT INTO main.\"{}\" SELECT * FROM src.\"{}\"",
                name, name
            ))?;
        }
        for obj_sql in &extra_objects {
            conn.execute_batch(obj_sql)?;
        }

        conn.execute_batch("COMMIT")?;
        Ok(())
    })();

    if result.is_err() {
        let _ = conn.execute_batch("ROLLBACK");
    }

    // Always restore FK checks and detach
    let _ = conn.execute_batch("PRAGMA foreign_keys = ON");
    let _ = conn.execute_batch("DETACH DATABASE src");

    result
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
        .filter_map(|r| r.ok())
        .collect();

    for table in &tables {
        conn.execute(&format!("DELETE FROM \"{}\"", table), [])?;
    }

    warn!(tables_cleared = tables.len(), "Database cleared");

    Ok(Html(String::new()))
}

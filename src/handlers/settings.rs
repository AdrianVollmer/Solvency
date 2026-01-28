use askama::Template;
use axum::extract::{Multipart, State};
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::Form;
use serde::Deserialize;
use std::fs;

use tracing::warn;

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
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

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

    let sql = generate_sql_dump(&conn)?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/sql"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"solvency-backup.sql\"",
            ),
        ],
        sql,
    ))
}

fn generate_sql_dump(conn: &rusqlite::Connection) -> AppResult<String> {
    let mut sql = String::new();

    sql.push_str("-- Solvency Database Backup\n");
    sql.push_str(&format!(
        "-- Generated at: {}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));

    // Get all user tables (excluding internal SQLite and migration tables)
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

    // For each table, get schema and data
    for table in &tables {
        // Get CREATE TABLE statement
        let create_sql: String = conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name=?",
            [table],
            |row| row.get(0),
        )?;

        sql.push_str(&format!("-- Table: {}\n", table));
        sql.push_str(&format!("DROP TABLE IF EXISTS \"{}\";\n", table));
        sql.push_str(&format!("{};\n\n", create_sql));

        // Get all rows from the table
        let row_sql = format!("SELECT * FROM \"{}\"", table);
        let mut data_stmt = conn.prepare(&row_sql)?;
        let column_count = data_stmt.column_count();
        let column_names: Vec<String> = data_stmt
            .column_names()
            .iter()
            .map(|s| format!("\"{}\"", s))
            .collect();

        let mut rows = data_stmt.query([])?;
        while let Some(row) = rows.next()? {
            let mut values: Vec<String> = Vec::with_capacity(column_count);
            for i in 0..column_count {
                let value = format_sql_value(row, i);
                values.push(value);
            }
            sql.push_str(&format!(
                "INSERT INTO \"{}\" ({}) VALUES ({});\n",
                table,
                column_names.join(", "),
                values.join(", ")
            ));
        }
        sql.push('\n');
    }

    Ok(sql)
}

fn format_sql_value(row: &rusqlite::Row, idx: usize) -> String {
    // Try different types in order
    if let Ok(val) = row.get::<_, Option<i64>>(idx) {
        match val {
            Some(v) => v.to_string(),
            None => "NULL".to_string(),
        }
    } else if let Ok(val) = row.get::<_, Option<f64>>(idx) {
        match val {
            Some(v) => v.to_string(),
            None => "NULL".to_string(),
        }
    } else if let Ok(val) = row.get::<_, Option<String>>(idx) {
        match val {
            Some(v) => format!("'{}'", v.replace('\'', "''")),
            None => "NULL".to_string(),
        }
    } else if let Ok(val) = row.get::<_, Option<Vec<u8>>>(idx) {
        match val {
            Some(v) => {
                let hex_str: String = v.iter().map(|b| format!("{:02x}", b)).collect();
                format!("X'{}'", hex_str)
            }
            None => "NULL".to_string(),
        }
    } else {
        "NULL".to_string()
    }
}

pub async fn import_database(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<Html<String>> {
    let mut sql_content = String::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read upload: {}", e)))?
    {
        if field.name() == Some("file") {
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::Internal(format!("Failed to read file: {}", e)))?;
            sql_content = String::from_utf8(bytes.to_vec())
                .map_err(|e| AppError::Validation(format!("Invalid UTF-8 in SQL file: {}", e)))?;
            break;
        }
    }

    if sql_content.is_empty() {
        return Err(AppError::Validation("No SQL file uploaded".to_string()));
    }

    // Execute the SQL statements
    let conn = state.db.get()?;
    conn.execute_batch(&sql_content)?;

    let template = SettingsSavedTemplate {
        icons: crate::filters::Icons,
        message: "Database imported successfully. Please refresh the page.".into(),
    };

    template.render_html()
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

    Ok(Html(String::new()))
}

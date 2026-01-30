use askama::Template;
use axum::extract::{Multipart, Path, Query, State};
use axum::response::{Html, Redirect};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::queries::trading;
use crate::error::{AppError, AppResult, RenderHtml};
use crate::models::{
    NewTradingActivity, Settings, TradingActivityType, TradingImportSession, TradingImportStatus,
};
use crate::services::trading_csv_parser::parse_csv;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

const PREVIEW_PAGE_SIZE: i64 = 50;

// Templates

#[derive(Template)]
#[template(path = "pages/trading_import.html")]
pub struct TradingImportTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
}

#[derive(Template)]
#[template(path = "pages/trading_import_format.html")]
pub struct TradingImportFormatTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub activity_types: &'static [TradingActivityType],
}

#[derive(Template)]
#[template(path = "pages/trading_import_wizard.html")]
pub struct TradingImportWizardTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub session: TradingImportSession,
}

#[derive(Template)]
#[template(path = "partials/trading_import_status.html")]
pub struct TradingImportStatusTemplate {
    pub icons: crate::filters::Icons,
    pub session: TradingImportSession,
}

#[derive(Template)]
#[template(path = "partials/trading_import_preview_table.html")]
pub struct TradingImportPreviewTableTemplate {
    pub session_id: String,
    pub rows: Vec<crate::models::TradingImportRow>,
    pub page: i64,
    pub page_size: i64,
    pub total_count: i64,
}

#[derive(Template)]
#[template(path = "partials/trading_import_result.html")]
pub struct TradingImportResultTemplate {
    pub icons: crate::filters::Icons,
    pub imported_count: i64,
    pub error_count: i64,
    pub errors: Vec<String>,
}

// Query params

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
}

// Status response for JSON endpoint

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub total_rows: i64,
    pub processed_rows: i64,
    pub error_count: i64,
    pub progress_percent: i64,
}

// Handlers

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let app_settings = state.load_settings()?;

    let template = TradingImportTemplate {
        title: "Import Trading Activities".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
    };

    template.render_html()
}

pub async fn format(State(state): State<AppState>) -> AppResult<Html<String>> {
    let app_settings = state.load_settings()?;

    let template = TradingImportFormatTemplate {
        title: "Trading CSV Format".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        activity_types: TradingActivityType::all(),
    };

    template.render_html()
}

pub async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<Redirect> {
    let session_id = Uuid::new_v4().to_string();

    // Create session
    {
        let conn = state.db.get()?;
        trading::create_import_session(&conn, &session_id)?;
    }

    // Collect files
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::CsvParse(e.to_string()))?
    {
        if field.name() == Some("files") {
            let file_name = field
                .file_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("file_{}", files.len() + 1));

            let content = field
                .bytes()
                .await
                .map_err(|e| AppError::CsvParse(e.to_string()))?
                .to_vec();

            if !content.is_empty() {
                files.push((file_name, content));
            }
        }
    }

    if files.is_empty() {
        let conn = state.db.get()?;
        trading::update_import_session_status(&conn, &session_id, TradingImportStatus::Failed)?;
        trading::update_import_session_errors(
            &conn,
            &session_id,
            1,
            &["No files uploaded".to_string()],
        )?;
        return Ok(Redirect::to(&format!("/trading/import/{}", session_id)));
    }

    // Spawn background parsing task
    let state_clone = state.clone();
    let session_id_clone = session_id.clone();

    tokio::spawn(async move {
        parse_files_background(state_clone, session_id_clone, files).await;
    });

    Ok(Redirect::to(&format!("/trading/import/{}", session_id)))
}

async fn parse_files_background(
    state: AppState,
    session_id: String,
    files: Vec<(String, Vec<u8>)>,
) {
    let mut all_errors: Vec<String> = Vec::new();
    let mut row_index: i64 = 0;

    for (file_name, content) in files {
        match parse_csv(&content) {
            Ok(result) => {
                // Insert rows into database
                if let Ok(conn) = state.db.get() {
                    for activity in result.activities {
                        if let Err(e) =
                            trading::insert_import_row(&conn, &session_id, row_index, &activity)
                        {
                            all_errors.push(format!("{}: Failed to store row: {}", file_name, e));
                        }
                        row_index += 1;

                        // Update progress periodically
                        if row_index % 100 == 0 {
                            let _ = trading::update_import_session_progress(
                                &conn,
                                &session_id,
                                row_index,
                                row_index,
                            );
                        }
                    }

                    for error in result.errors {
                        all_errors.push(format!("{}: {}", file_name, error));
                    }
                }
            }
            Err(e) => {
                all_errors.push(format!("{}: {}", file_name, e));
            }
        }
    }

    // Finalize session
    if let Ok(conn) = state.db.get() {
        let _ = trading::update_import_session_progress(&conn, &session_id, row_index, row_index);
        let _ = trading::update_import_session_errors(
            &conn,
            &session_id,
            all_errors.len() as i64,
            &all_errors,
        );

        if row_index == 0 && !all_errors.is_empty() {
            let _ = trading::update_import_session_status(
                &conn,
                &session_id,
                TradingImportStatus::Failed,
            );
        } else {
            let _ = trading::update_import_session_status(
                &conn,
                &session_id,
                TradingImportStatus::Preview,
            );
        }
    }
}

pub async fn wizard(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let session = trading::get_import_session(&conn, &session_id)?;
    let app_settings = state.load_settings()?;

    let template = TradingImportWizardTemplate {
        title: "Import Trading Activities".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        session,
    };

    template.render_html()
}

pub async fn status(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let session = trading::get_import_session(&conn, &session_id)?;

    let template = TradingImportStatusTemplate {
        icons: crate::filters::Icons,
        session,
    };
    template.render_html()
}

pub async fn status_json(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<axum::Json<StatusResponse>> {
    let conn = state.db.get()?;
    let session = trading::get_import_session(&conn, &session_id)?;

    Ok(axum::Json(StatusResponse {
        status: session.status.as_str().to_string(),
        total_rows: session.total_rows,
        processed_rows: session.processed_rows,
        error_count: session.error_count,
        progress_percent: session.progress_percent(),
    }))
}

pub async fn rows(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<PageQuery>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * PREVIEW_PAGE_SIZE;

    let rows = trading::get_import_rows_paginated(&conn, &session_id, PREVIEW_PAGE_SIZE, offset)?;
    let total_count = trading::count_import_rows(&conn, &session_id)?;

    let template = TradingImportPreviewTableTemplate {
        session_id,
        rows,
        page,
        page_size: PREVIEW_PAGE_SIZE,
        total_count,
    };

    template.render_html()
}

pub async fn confirm(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Html<String>> {
    // Update status to importing
    {
        let conn = state.db.get()?;
        let session = trading::get_import_session(&conn, &session_id)?;

        if session.status != TradingImportStatus::Preview {
            return Err(AppError::Validation(
                "Session is not ready for import".into(),
            ));
        }

        trading::update_import_session_status(&conn, &session_id, TradingImportStatus::Importing)?;
        trading::update_import_session_progress(&conn, &session_id, session.total_rows, 0)?;
    }

    // Spawn background import task
    let state_clone = state.clone();
    let session_id_clone = session_id.clone();

    tokio::spawn(async move {
        import_rows_background(state_clone, session_id_clone).await;
    });

    // Return status template for polling
    let conn = state.db.get()?;
    let session = trading::get_import_session(&conn, &session_id)?;

    let template = TradingImportStatusTemplate {
        icons: crate::filters::Icons,
        session,
    };
    template.render_html()
}

async fn import_rows_background(state: AppState, session_id: String) {
    let pending_rows = {
        let conn = match state.db.get() {
            Ok(c) => c,
            Err(_) => return,
        };
        match trading::get_pending_import_rows(&conn, &session_id) {
            Ok(r) => r,
            Err(_) => return,
        }
    };

    let mut error_count = 0;
    let mut errors: Vec<String> = Vec::new();

    for row in pending_rows {
        let conn = match state.db.get() {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Parse activity type
        let activity_type: TradingActivityType = match row.data.activity_type.parse() {
            Ok(t) => t,
            Err(_) => {
                error_count += 1;
                errors.push(format!(
                    "Row {}: Invalid activity type '{}'",
                    row.row_index + 1,
                    row.data.activity_type
                ));
                let _ = trading::mark_import_row_error(&conn, row.id, "Invalid activity type");
                let _ = trading::increment_import_session_processed(&conn, &session_id);
                let _ = trading::increment_import_session_error_count(&conn, &session_id);
                continue;
            }
        };

        // Parse quantity
        let quantity: Option<f64> = match &row.data.quantity {
            Some(q) => match q.parse() {
                Ok(v) => Some(v),
                Err(_) => {
                    error_count += 1;
                    errors.push(format!(
                        "Row {}: Invalid quantity '{}'",
                        row.row_index + 1,
                        q
                    ));
                    let _ = trading::mark_import_row_error(&conn, row.id, "Invalid quantity");
                    let _ = trading::increment_import_session_processed(&conn, &session_id);
                    let _ = trading::increment_import_session_error_count(&conn, &session_id);
                    continue;
                }
            },
            None => None,
        };

        // Parse unit price
        let unit_price_cents: Option<i64> = match &row.data.unit_price {
            Some(p) => match p.parse::<f64>() {
                Ok(v) => Some((v * 100.0).round() as i64),
                Err(_) => {
                    error_count += 1;
                    errors.push(format!(
                        "Row {}: Invalid unit price '{}'",
                        row.row_index + 1,
                        p
                    ));
                    let _ = trading::mark_import_row_error(&conn, row.id, "Invalid unit price");
                    let _ = trading::increment_import_session_processed(&conn, &session_id);
                    let _ = trading::increment_import_session_error_count(&conn, &session_id);
                    continue;
                }
            },
            None => None,
        };

        // Parse fee
        let fee_cents: i64 = match &row.data.fee {
            Some(f) => match f.parse::<f64>() {
                Ok(v) => (v * 100.0).round() as i64,
                Err(_) => {
                    error_count += 1;
                    errors.push(format!("Row {}: Invalid fee '{}'", row.row_index + 1, f));
                    let _ = trading::mark_import_row_error(&conn, row.id, "Invalid fee");
                    let _ = trading::increment_import_session_processed(&conn, &session_id);
                    let _ = trading::increment_import_session_error_count(&conn, &session_id);
                    continue;
                }
            },
            None => 0,
        };

        let new_activity = NewTradingActivity {
            date: row.data.date.clone(),
            symbol: row.data.symbol.clone(),
            quantity,
            activity_type,
            unit_price_cents,
            currency: row.data.currency.clone(),
            fee_cents,
            account_id: row.data.account_id,
            notes: None,
        };

        match trading::create_activity(&conn, &new_activity) {
            Ok(id) => {
                // Apply split adjustments for the newly imported activity.
                let split_result = match activity_type {
                    TradingActivityType::Split => {
                        if let Some(ratio) = quantity {
                            trading::apply_split_to_past_activities(
                                &conn,
                                id,
                                &new_activity.symbol,
                                &new_activity.date,
                                ratio,
                            )
                        } else {
                            Ok(())
                        }
                    }
                    TradingActivityType::Buy | TradingActivityType::Sell => {
                        trading::apply_existing_splits_to_activity(
                            &conn,
                            id,
                            &new_activity.symbol,
                            &new_activity.date,
                        )
                    }
                    _ => Ok(()),
                };
                if let Err(e) = split_result {
                    error_count += 1;
                    errors.push(format!(
                        "Row {}: Split adjustment failed: {}",
                        row.row_index + 1,
                        e
                    ));
                    let _ = trading::mark_import_row_error(
                        &conn,
                        row.id,
                        &format!("Split adjustment failed: {}", e),
                    );
                    let _ = trading::increment_import_session_error_count(&conn, &session_id);
                } else {
                    let _ = trading::mark_import_row_imported(&conn, row.id);
                }
            }
            Err(e) => {
                error_count += 1;
                errors.push(format!("Row {}: {}", row.row_index + 1, e));
                let _ = trading::mark_import_row_error(&conn, row.id, &e.to_string());
                let _ = trading::increment_import_session_error_count(&conn, &session_id);
            }
        }

        let _ = trading::increment_import_session_processed(&conn, &session_id);
    }

    // Finalize
    if let Ok(conn) = state.db.get() {
        let _ = trading::update_import_session_errors(&conn, &session_id, error_count, &errors);
        let _ = trading::update_import_session_status(
            &conn,
            &session_id,
            TradingImportStatus::Completed,
        );
    }
}

pub async fn result(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let session = trading::get_import_session(&conn, &session_id)?;

    let template = TradingImportResultTemplate {
        icons: crate::filters::Icons,
        imported_count: session.processed_rows - session.error_count,
        error_count: session.error_count,
        errors: session.errors,
    };

    template.render_html()
}

pub async fn cancel(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;
    trading::delete_import_session(&conn, &session_id)?;
    Ok(Redirect::to("/trading/import"))
}

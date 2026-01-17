use askama::Template;
use axum::extract::{Multipart, Path, Query, State};
use axum::response::{Html, Redirect};
use axum::Form;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::queries::{categories, expenses, import, settings, tags};
use crate::error::{html_escape, AppError, AppResult};
use crate::models::{CategoryWithPath, ImportSession, ImportStatus, NewExpense, Settings};
use crate::services::csv_parser::parse_csv;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

const PREVIEW_PAGE_SIZE: i64 = 50;

// Templates

#[derive(Template)]
#[template(path = "pages/import.html")]
pub struct ImportTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
}

#[derive(Template)]
#[template(path = "pages/import_format.html")]
pub struct ImportFormatTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
}

#[derive(Template)]
#[template(path = "pages/import_wizard.html")]
pub struct ImportWizardTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub session: ImportSession,
    pub categories: Vec<CategoryWithPath>,
}

#[derive(Template)]
#[template(path = "partials/import_status.html")]
pub struct ImportStatusTemplate {
    pub session: ImportSession,
    pub categories: Vec<CategoryWithPath>,
}

#[derive(Template)]
#[template(path = "partials/import_preview_table.html")]
pub struct ImportPreviewTableTemplate {
    pub session_id: String,
    pub rows: Vec<crate::models::ImportRow>,
    pub categories: Vec<CategoryWithPath>,
    pub page: i64,
    pub page_size: i64,
    pub total_count: i64,
}

#[derive(Template)]
#[template(path = "partials/import_result.html")]
pub struct ImportResultTemplate {
    pub imported_count: i64,
    pub error_count: i64,
    pub errors: Vec<String>,
}

// Query params

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CategoryForm {
    pub category_id: Option<i64>,
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
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

    let template = ImportTemplate {
        title: "Import".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        version: VERSION,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn format(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

    let template = ImportFormatTemplate {
        title: "CSV Import Format".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        version: VERSION,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<Redirect> {
    let session_id = Uuid::new_v4().to_string();

    // Create session
    {
        let conn = state.db.get()?;
        import::create_session(&conn, &session_id)?;
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
        import::update_session_status(&conn, &session_id, ImportStatus::Failed)?;
        import::update_session_errors(&conn, &session_id, 1, &["No files uploaded".to_string()])?;
        return Ok(Redirect::to(&format!("/import/{}", session_id)));
    }

    // Spawn background parsing task
    let state_clone = state.clone();
    let session_id_clone = session_id.clone();

    tokio::spawn(async move {
        parse_files_background(state_clone, session_id_clone, files).await;
    });

    Ok(Redirect::to(&format!("/import/{}", session_id)))
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
                    for expense in result.expenses {
                        if let Err(e) = import::insert_row(&conn, &session_id, row_index, &expense)
                        {
                            all_errors.push(format!("{}: Failed to store row: {}", file_name, e));
                        }
                        row_index += 1;

                        // Update progress periodically
                        if row_index % 100 == 0 {
                            let _ = import::update_session_progress(
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
        let _ = import::update_session_progress(&conn, &session_id, row_index, row_index);
        let _ =
            import::update_session_errors(&conn, &session_id, all_errors.len() as i64, &all_errors);

        if row_index == 0 && !all_errors.is_empty() {
            let _ = import::update_session_status(&conn, &session_id, ImportStatus::Failed);
        } else {
            let _ = import::update_session_status(&conn, &session_id, ImportStatus::Preview);
        }
    }
}

pub async fn wizard(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let session = import::get_session(&conn, &session_id)?;
    let app_settings = settings::get_settings(&conn)?;
    let cats = categories::list_categories_with_path(&conn)?;

    let template = ImportWizardTemplate {
        title: "Import".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        version: VERSION,
        session,
        categories: cats,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn status(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let session = import::get_session(&conn, &session_id)?;
    let cats = categories::list_categories_with_path(&conn)?;

    let template = ImportStatusTemplate {
        session,
        categories: cats,
    };
    Ok(Html(template.render().unwrap()))
}

pub async fn status_json(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<axum::Json<StatusResponse>> {
    let conn = state.db.get()?;
    let session = import::get_session(&conn, &session_id)?;

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

    let rows = import::get_rows_paginated(&conn, &session_id, PREVIEW_PAGE_SIZE, offset)?;
    let total_count = import::count_rows(&conn, &session_id)?;
    let cats = categories::list_categories_with_path(&conn)?;

    let template = ImportPreviewTableTemplate {
        session_id,
        rows,
        categories: cats,
        page,
        page_size: PREVIEW_PAGE_SIZE,
        total_count,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn update_row_category(
    State(state): State<AppState>,
    Path((_session_id, row_id)): Path<(String, i64)>,
    Form(form): Form<CategoryForm>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    import::update_row_category(&conn, row_id, form.category_id)?;

    // Return updated category display
    let cat_name = if let Some(cat_id) = form.category_id {
        categories::get_category(&conn, cat_id)
            .ok()
            .flatten()
            .map(|c| c.name)
            .unwrap_or_default()
    } else {
        String::new()
    };

    let display_name = if cat_name.is_empty() {
        "Uncategorized".to_string()
    } else {
        html_escape(&cat_name)
    };

    Ok(Html(format!(
        r#"<span class="text-gray-600 dark:text-gray-400">{}</span>"#,
        display_name
    )))
}

pub async fn update_all_categories(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Form(form): Form<CategoryForm>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;
    import::update_all_rows_category(&conn, &session_id, form.category_id)?;

    Ok(Redirect::to(&format!("/import/{}", session_id)))
}

pub async fn confirm(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Html<String>> {
    // Update status to importing
    {
        let conn = state.db.get()?;
        let session = import::get_session(&conn, &session_id)?;

        if session.status != ImportStatus::Preview {
            return Err(AppError::Validation(
                "Session is not ready for import".into(),
            ));
        }

        import::update_session_status(&conn, &session_id, ImportStatus::Importing)?;
        import::update_session_progress(&conn, &session_id, session.total_rows, 0)?;
    }

    // Spawn background import task
    let state_clone = state.clone();
    let session_id_clone = session_id.clone();

    tokio::spawn(async move {
        import_rows_background(state_clone, session_id_clone).await;
    });

    // Return status template for polling
    let conn = state.db.get()?;
    let session = import::get_session(&conn, &session_id)?;
    let cats = categories::list_categories_with_path(&conn)?;

    let template = ImportStatusTemplate {
        session,
        categories: cats,
    };
    Ok(Html(template.render().unwrap()))
}

async fn import_rows_background(state: AppState, session_id: String) {
    let conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return,
    };

    let pending_rows = match import::get_pending_rows(&conn, &session_id) {
        Ok(r) => r,
        Err(_) => return,
    };

    let mut error_count = 0;
    let mut errors: Vec<String> = Vec::new();

    for row in pending_rows {
        let amount: f64 = match row.data.amount.parse() {
            Ok(a) => a,
            Err(_) => {
                error_count += 1;
                errors.push(format!(
                    "Row {}: Invalid amount '{}'",
                    row.row_index + 1,
                    row.data.amount
                ));
                let _ = import::mark_row_error(&conn, row.id, "Invalid amount");
                let _ = import::increment_session_processed(&conn, &session_id);
                let _ = import::increment_session_error_count(&conn, &session_id);
                continue;
            }
        };

        // Handle tags
        let tag_ids: Vec<i64> = row
            .data
            .tags
            .iter()
            .filter_map(|name| {
                let name = name.trim();
                if name.is_empty() {
                    return None;
                }
                tags::create_or_get_tag(&conn, name).ok().map(|t| t.id)
            })
            .collect();

        let new_expense = NewExpense {
            date: row.data.date.clone(),
            amount_cents: (amount * 100.0).round() as i64,
            currency: row.data.currency.clone(),
            description: row.data.description.clone(),
            category_id: row.category_id,
            notes: row.data.notes.clone(),
            tag_ids,
            value_date: row.data.value_date.clone(),
            payer: row.data.payer.clone(),
            payee: row.data.payee.clone(),
            reference: row.data.reference.clone(),
            transaction_type: row.data.transaction_type.clone(),
            counterparty_iban: row.data.counterparty_iban.clone(),
            creditor_id: row.data.creditor_id.clone(),
            mandate_reference: row.data.mandate_reference.clone(),
            customer_reference: row.data.customer_reference.clone(),
        };

        match expenses::create_expense(&conn, &new_expense) {
            Ok(_) => {
                let _ = import::mark_row_imported(&conn, row.id);
            }
            Err(e) => {
                error_count += 1;
                errors.push(format!("Row {}: {}", row.row_index + 1, e));
                let _ = import::mark_row_error(&conn, row.id, &e.to_string());
                let _ = import::increment_session_error_count(&conn, &session_id);
            }
        }

        let _ = import::increment_session_processed(&conn, &session_id);
    }

    // Finalize
    let _ = import::update_session_errors(&conn, &session_id, error_count, &errors);
    let _ = import::update_session_status(&conn, &session_id, ImportStatus::Completed);
}

pub async fn result(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let session = import::get_session(&conn, &session_id)?;

    let template = ImportResultTemplate {
        imported_count: session.processed_rows - session.error_count,
        error_count: session.error_count,
        errors: session.errors,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn cancel(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;
    import::delete_session(&conn, &session_id)?;
    Ok(Redirect::to("/import"))
}

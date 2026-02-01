use askama::Template;
use axum::extract::{Multipart, Path, Query, State};
use axum::response::{Html, Redirect};
use axum::Form;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

use regex::RegexBuilder;

use crate::db::queries::{categories, import, rules, tags, transactions};
use crate::error::{html_escape, AppError, AppResult, RenderHtml};
use crate::models::{
    CategoryWithPath, ImportSession, ImportStatus, NewTransaction, RuleActionType, Settings,
};
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
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
}

#[derive(Template)]
#[template(path = "pages/import_format.html")]
pub struct ImportFormatTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
}

#[derive(Template)]
#[template(path = "pages/import_wizard.html")]
pub struct ImportWizardTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub session: ImportSession,
    pub categories: Vec<CategoryWithPath>,
}

#[derive(Template)]
#[template(path = "partials/import_status.html")]
pub struct ImportStatusTemplate {
    pub icons: crate::filters::Icons,
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

#[derive(Debug, Deserialize)]
pub struct CategoryForm {
    #[serde(
        default,
        deserialize_with = "crate::form_utils::deserialize_optional_i64"
    )]
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
    let app_settings = state.load_settings()?;

    let template = ImportTemplate {
        title: "Import".into(),
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

    let template = ImportFormatTemplate {
        title: "CSV Import Format".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
    };

    template.render_html()
}

pub async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<Redirect> {
    let session_id = Uuid::new_v4().to_string();
    info!(session_id = %session_id, "Starting CSV upload");

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
                debug!(file_name = %file_name, size_bytes = content.len(), "Received CSV file");
                files.push((file_name, content));
            }
        }
    }

    if files.is_empty() {
        warn!(session_id = %session_id, "No files uploaded");
        let conn = state.db.get()?;
        import::update_session_status(&conn, &session_id, ImportStatus::Failed)?;
        import::update_session_errors(&conn, &session_id, 1, &["No files uploaded".to_string()])?;
        return Ok(Redirect::to(&format!("/import/{}", session_id)));
    }

    info!(session_id = %session_id, file_count = files.len(), "Processing uploaded files");

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
    debug!(session_id = %session_id, file_count = files.len(), "Starting background CSV parsing");
    let mut all_errors: Vec<String> = Vec::new();
    let mut row_index: i64 = 0;

    for (file_name, content) in &files {
        debug!(session_id = %session_id, file_name = %file_name, "Parsing CSV file");
        match parse_csv(content) {
            Ok(result) => {
                debug!(
                    file_name = %file_name,
                    rows_parsed = result.transactions.len(),
                    parse_errors = result.errors.len(),
                    "CSV file parsed"
                );
                // Insert rows into database
                if let Ok(conn) = state.db.get() {
                    for transaction in result.transactions {
                        if let Err(e) =
                            import::insert_row(&conn, &session_id, row_index, &transaction)
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
                warn!(file_name = %file_name, error = %e, "Failed to parse CSV file");
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
            warn!(session_id = %session_id, "Import failed - no valid rows parsed");
            let _ = import::update_session_status(&conn, &session_id, ImportStatus::Failed);
        } else {
            info!(
                session_id = %session_id,
                total_rows = row_index,
                error_count = all_errors.len(),
                "CSV parsing completed, applying rules"
            );
            apply_rules_to_import_rows(&conn, &session_id);
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
    let app_settings = state.load_settings()?;
    let cats = state.cached_categories_with_path()?;

    let template = ImportWizardTemplate {
        title: "Import".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        session,
        categories: cats,
    };

    template.render_html()
}

pub async fn status(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let session = import::get_session(&conn, &session_id)?;
    let cats = state.cached_categories_with_path()?;

    let template = ImportStatusTemplate {
        icons: crate::filters::Icons,
        session,
        categories: cats,
    };
    template.render_html()
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
    let cats = state.cached_categories_with_path()?;

    let template = ImportPreviewTableTemplate {
        session_id,
        rows,
        categories: cats,
        page,
        page_size: PREVIEW_PAGE_SIZE,
        total_count,
    };

    template.render_html()
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
    info!(session_id = %session_id, "Confirming import");

    // Update status to importing
    {
        let conn = state.db.get()?;
        let session = import::get_session(&conn, &session_id)?;

        if session.status != ImportStatus::Preview {
            warn!(session_id = %session_id, status = %session.status.as_str(), "Import session not ready");
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
    let cats = state.cached_categories_with_path()?;

    let template = ImportStatusTemplate {
        icons: crate::filters::Icons,
        session,
        categories: cats,
    };
    template.render_html()
}

fn apply_rules_to_import_rows(conn: &rusqlite::Connection, session_id: &str) {
    let all_rules = match rules::list_rules(conn) {
        Ok(r) => r,
        Err(e) => {
            warn!(session_id = %session_id, error = %e, "Failed to load rules for import");
            return;
        }
    };

    if all_rules.is_empty() {
        return;
    }

    struct CompiledRule {
        regex: regex::Regex,
        action_type: RuleActionType,
        category_id: Option<i64>,
        tag_name: Option<String>,
    }

    let compiled: Vec<CompiledRule> = all_rules
        .iter()
        .filter_map(|rule| {
            let regex = RegexBuilder::new(&rule.pattern)
                .case_insensitive(true)
                .build()
                .ok()?;

            match rule.action_type {
                RuleActionType::AssignCategory => {
                    let cat_id: i64 = rule.action_value.parse().ok()?;
                    Some(CompiledRule {
                        regex,
                        action_type: rule.action_type,
                        category_id: Some(cat_id),
                        tag_name: None,
                    })
                }
                RuleActionType::AssignTag => {
                    let tag_id: i64 = rule.action_value.parse().ok()?;
                    let tag = tags::get_tag(conn, tag_id).ok()??;
                    Some(CompiledRule {
                        regex,
                        action_type: rule.action_type,
                        category_id: None,
                        tag_name: Some(tag.name),
                    })
                }
            }
        })
        .collect();

    if compiled.is_empty() {
        return;
    }

    let rows = match import::get_pending_rows(conn, session_id) {
        Ok(r) => r,
        Err(e) => {
            warn!(session_id = %session_id, error = %e, "Failed to load rows for rule application");
            return;
        }
    };

    let mut affected = 0u64;

    for row in &rows {
        let mut matched_category: Option<i64> = None;
        let mut extra_tags: Vec<String> = Vec::new();

        for cr in &compiled {
            if !cr.regex.is_match(&row.data.description) {
                continue;
            }
            match cr.action_type {
                RuleActionType::AssignCategory => {
                    if matched_category.is_none() {
                        matched_category = cr.category_id;
                    }
                }
                RuleActionType::AssignTag => {
                    if let Some(ref name) = cr.tag_name {
                        if !row.data.tags.contains(name) && !extra_tags.contains(name) {
                            extra_tags.push(name.clone());
                        }
                    }
                }
            }
        }

        let has_changes = matched_category.is_some() || !extra_tags.is_empty();
        if !has_changes {
            continue;
        }
        affected += 1;

        if let Some(cat_id) = matched_category {
            let _ = import::update_row_category(conn, row.id, Some(cat_id));
        }

        if !extra_tags.is_empty() {
            let mut data = row.data.clone();
            data.tags.extend(extra_tags);
            let _ = import::update_row_data(conn, row.id, &data);
        }
    }

    info!(
        session_id = %session_id,
        rules_compiled = compiled.len(),
        rows_affected = affected,
        "Applied rules to import rows"
    );
}

async fn import_rows_background(state: AppState, session_id: String) {
    debug!(session_id = %session_id, "Starting background import");

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(session_id = %session_id, error = %e, "Failed to get database connection");
            return;
        }
    };

    let pending_rows = match import::get_pending_rows(&conn, &session_id) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(session_id = %session_id, error = %e, "Failed to get pending rows");
            return;
        }
    };

    info!(session_id = %session_id, row_count = pending_rows.len(), "Importing rows");

    let mut error_count = 0;
    let mut errors: Vec<String> = Vec::new();
    const BATCH_SIZE: usize = 100;

    let mut tx = match conn.transaction() {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(session_id = %session_id, error = %e, "Failed to start transaction");
            return;
        }
    };

    for (i, row) in pending_rows.into_iter().enumerate() {
        // Commit in batches for performance while keeping progress visible
        if i > 0 && i % BATCH_SIZE == 0 {
            if let Err(e) = tx.commit() {
                tracing::error!(session_id = %session_id, error = %e, "Failed to commit batch");
                return;
            }
            tx = match conn.transaction() {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!(session_id = %session_id, error = %e, "Failed to start transaction");
                    return;
                }
            };
        }

        let amount: f64 = match row.data.amount.parse() {
            Ok(a) => a,
            Err(_) => {
                error_count += 1;
                errors.push(format!(
                    "Row {}: Invalid amount '{}'",
                    row.row_index + 1,
                    row.data.amount
                ));
                let _ = import::mark_row_error(&tx, row.id, "Invalid amount");
                let _ = import::increment_session_processed(&tx, &session_id);
                let _ = import::increment_session_error_count(&tx, &session_id);
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
                tags::create_or_get_tag(&tx, name).ok().map(|t| t.id)
            })
            .collect();

        let new_transaction = NewTransaction {
            date: row.data.date.clone(),
            amount_cents: (amount * 100.0).round() as i64,
            currency: row.data.currency.clone(),
            description: row.data.description.clone(),
            category_id: row.category_id,
            account_id: row.data.account_id,
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

        match transactions::create_transaction(&tx, &new_transaction) {
            Ok(_) => {
                let _ = import::mark_row_imported(&tx, row.id);
            }
            Err(e) => {
                error_count += 1;
                errors.push(format!("Row {}: {}", row.row_index + 1, e));
                let _ = import::mark_row_error(&tx, row.id, &e.to_string());
                let _ = import::increment_session_error_count(&tx, &session_id);
            }
        }

        let _ = import::increment_session_processed(&tx, &session_id);
    }

    // Commit final batch
    if let Err(e) = tx.commit() {
        tracing::error!(session_id = %session_id, error = %e, "Failed to commit final batch");
        return;
    }

    // Finalize
    let _ = import::update_session_errors(&conn, &session_id, error_count, &errors);
    let _ = import::update_session_status(&conn, &session_id, ImportStatus::Completed);

    info!(
        session_id = %session_id,
        error_count = error_count,
        "Import completed"
    );
}

pub async fn result(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let session = import::get_session(&conn, &session_id)?;

    let template = ImportResultTemplate {
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
    info!(session_id = %session_id, "Cancelling import");
    let conn = state.db.get()?;
    import::delete_session(&conn, &session_id)?;
    Ok(Redirect::to("/import"))
}

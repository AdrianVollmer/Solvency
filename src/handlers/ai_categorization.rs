use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect};
use axum::Form;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::queries::{ai_categorization, api_logs, settings, transactions};
use crate::error::{AppError, AppResult, RenderHtml};
use crate::models::ai_categorization::{
    AiCategorizationResultWithDetails, AiCategorizationSession, AiCategorizationStatus, AiProvider,
    AiResultStatus, AiSettings,
};
use crate::models::{NewApiLog, Settings};
use crate::services::ai_client::{
    categorize_transactions, test_connection, CategoryOption, TransactionForCategorization,
};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

const RESULTS_PAGE_SIZE: i64 = 50;
const BATCH_SIZE: usize = 5;
const RATE_LIMIT_DELAY_MS: u64 = 500;

// Templates

#[derive(Template)]
#[template(path = "pages/ai_categorization.html")]
pub struct AiCategorizationTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub ai_settings: AiSettings,
    pub providers: Vec<(AiProvider, &'static str)>,
    pub uncategorized_count: i64,
    pub total_count: i64,
}

#[derive(Template)]
#[template(path = "pages/ai_categorization_wizard.html")]
pub struct AiCategorizationWizardTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub session: AiCategorizationSession,
}

#[derive(Template)]
#[template(path = "partials/ai_categorization_status.html")]
pub struct AiCategorizationStatusTemplate {
    pub icons: crate::filters::Icons,
    pub session: AiCategorizationSession,
    pub settings: Settings,
}

#[derive(Template)]
#[template(path = "partials/ai_categorization_results.html")]
pub struct AiCategorizationResultsTemplate {
    pub session_id: String,
    pub results: Vec<AiCategorizationResultWithDetails>,
    pub page: i64,
    pub page_size: i64,
    pub total_count: i64,
    pub pending_count: i64,
    pub settings: Settings,
}

#[derive(Template)]
#[template(path = "partials/ai_test_result.html")]
pub struct AiTestResultTemplate {
    pub icons: crate::filters::Icons,
    pub success: bool,
    pub message: String,
    pub model_info: Option<String>,
}

#[derive(Template)]
#[template(path = "partials/ai_settings_saved.html")]
pub struct AiSettingsSavedTemplate {
    pub icons: crate::filters::Icons,
    pub message: String,
}

// Form structs

#[derive(Debug, Deserialize)]
pub struct SettingsForm {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Deserialize)]
pub struct StartForm {
    #[serde(default)]
    pub include_categorized: bool,
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
}

// Status response for JSON endpoint
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub total_transactions: i64,
    pub processed_transactions: i64,
    pub categorized_count: i64,
    pub skipped_count: i64,
    pub error_count: i64,
    pub progress_percent: i64,
}

// Handlers

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let app_settings = state.load_settings()?;
    let conn = state.db.get()?;

    let all_settings = settings::get_all_settings(&conn)?;
    let ai_settings = AiSettings::from_settings(&all_settings);

    let uncategorized_count = transactions::count_uncategorized(&conn)?;
    let total_count = transactions::count_all(&conn)?;

    let providers = vec![
        (AiProvider::Ollama, AiProvider::Ollama.label()),
        (AiProvider::OpenAi, AiProvider::OpenAi.label()),
        (AiProvider::Anthropic, AiProvider::Anthropic.label()),
    ];

    let template = AiCategorizationTemplate {
        title: "AI Categorization".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        ai_settings,
        providers,
        uncategorized_count,
        total_count,
    };

    template.render_html()
}

pub async fn save_settings(
    State(state): State<AppState>,
    Form(form): Form<SettingsForm>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    settings::set_setting(&conn, "ai_provider", &form.provider)?;
    settings::set_setting(&conn, "ai_base_url", &form.base_url)?;
    settings::set_setting(&conn, "ai_api_key", &form.api_key)?;
    settings::set_setting(&conn, "ai_model", &form.model)?;

    info!(provider = %form.provider, model = %form.model, "Saved AI settings");

    let template = AiSettingsSavedTemplate {
        icons: crate::filters::Icons,
        message: "Settings saved successfully".to_string(),
    };

    template.render_html()
}

pub async fn test(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let all_settings = settings::get_all_settings(&conn)?;
    let ai_settings = AiSettings::from_settings(&all_settings);

    if !ai_settings.is_configured() {
        let template = AiTestResultTemplate {
            icons: crate::filters::Icons,
            success: false,
            message: "Please configure AI settings first".to_string(),
            model_info: None,
        };
        return template.render_html();
    }

    let result = test_connection(&ai_settings).await;

    let template = AiTestResultTemplate {
        icons: crate::filters::Icons,
        success: result.success,
        message: result.message,
        model_info: result.model_info,
    };

    template.render_html()
}

pub async fn start(
    State(state): State<AppState>,
    Form(form): Form<StartForm>,
) -> AppResult<impl IntoResponse> {
    let session_id = Uuid::new_v4().to_string();
    info!(session_id = %session_id, include_categorized = form.include_categorized, "Starting AI categorization session");

    let conn = state.db.get()?;

    // Load settings
    let all_settings = settings::get_all_settings(&conn)?;
    let ai_settings = AiSettings::from_settings(&all_settings);

    if !ai_settings.is_configured() {
        return Err(AppError::Validation(
            "Please configure AI settings first".into(),
        ));
    }

    // Count transactions to process
    let total = if form.include_categorized {
        transactions::count_all(&conn)?
    } else {
        transactions::count_uncategorized(&conn)?
    };

    if total == 0 {
        return Err(AppError::Validation("No transactions to categorize".into()));
    }

    // Create session
    ai_categorization::create_session(
        &conn,
        &session_id,
        ai_settings.provider.as_str(),
        &ai_settings.model,
        total,
    )?;

    // Spawn background processing
    let state_clone = state.clone();
    let session_id_clone = session_id.clone();

    tokio::spawn(async move {
        process_categorization_background(
            state_clone,
            session_id_clone,
            ai_settings,
            form.include_categorized,
        )
        .await;
    });

    Ok(Redirect::to(&format!("/ai-categorization/{}", session_id)))
}

pub async fn wizard(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let session = ai_categorization::get_session(&conn, &session_id)?;
    let app_settings = state.load_settings()?;

    let template = AiCategorizationWizardTemplate {
        title: "AI Categorization".into(),
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
    let session = ai_categorization::get_session(&conn, &session_id)?;
    let app_settings = state.load_settings()?;

    let template = AiCategorizationStatusTemplate {
        icons: crate::filters::Icons,
        session,
        settings: app_settings,
    };
    template.render_html()
}

pub async fn status_json(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<axum::Json<StatusResponse>> {
    let conn = state.db.get()?;
    let session = ai_categorization::get_session(&conn, &session_id)?;

    Ok(axum::Json(StatusResponse {
        status: session.status.as_str().to_string(),
        total_transactions: session.total_transactions,
        processed_transactions: session.processed_transactions,
        categorized_count: session.categorized_count,
        skipped_count: session.skipped_count,
        error_count: session.error_count,
        progress_percent: session.progress_percent(),
    }))
}

pub async fn results(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<PageQuery>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * RESULTS_PAGE_SIZE;

    let results =
        ai_categorization::get_results_with_details(&conn, &session_id, RESULTS_PAGE_SIZE, offset)?;
    let total_count = ai_categorization::count_results(&conn, &session_id)?;
    let pending_count = ai_categorization::count_pending_results(&conn, &session_id)?;

    let template = AiCategorizationResultsTemplate {
        session_id,
        results,
        page,
        page_size: RESULTS_PAGE_SIZE,
        total_count,
        pending_count,
        settings: app_settings,
    };

    template.render_html()
}

pub async fn apply_result(
    State(state): State<AppState>,
    Path((_session_id, result_id)): Path<(String, i64)>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let applied = ai_categorization::apply_result(&conn, result_id)?;

    if applied {
        Ok(Html(
            r#"<span class="text-green-600 dark:text-green-400 text-sm">Applied</span>"#
                .to_string(),
        ))
    } else {
        Ok(Html(
            r#"<span class="text-gray-500 text-sm">No suggestion</span>"#.to_string(),
        ))
    }
}

pub async fn reject_result(
    State(state): State<AppState>,
    Path((_session_id, result_id)): Path<(String, i64)>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    ai_categorization::reject_result(&conn, result_id)?;

    Ok(Html(
        r#"<span class="text-gray-500 text-sm">Rejected</span>"#.to_string(),
    ))
}

pub async fn apply_all(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let conn = state.db.get()?;

    let applied = ai_categorization::apply_all_pending_results(&conn, &session_id)?;
    info!(session_id = %session_id, applied = applied, "Applied all pending AI suggestions");

    Ok(Redirect::to(&format!("/ai-categorization/{}", session_id)))
}

pub async fn cancel(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let conn = state.db.get()?;

    // Update status to cancelled
    ai_categorization::update_session_status(
        &conn,
        &session_id,
        AiCategorizationStatus::Cancelled,
    )?;
    info!(session_id = %session_id, "Cancelled AI categorization session");

    Ok(Redirect::to("/ai-categorization"))
}

// Background processing

async fn process_categorization_background(
    state: AppState,
    session_id: String,
    ai_settings: AiSettings,
    include_categorized: bool,
) {
    debug!(session_id = %session_id, "Starting background AI categorization");

    // Update status to processing
    if let Ok(conn) = state.db.get() {
        let _ = ai_categorization::update_session_status(
            &conn,
            &session_id,
            AiCategorizationStatus::Processing,
        );
    }

    // Load categories
    let categories_with_path = match state.cached_categories_with_path() {
        Ok(c) => c,
        Err(e) => {
            error!(session_id = %session_id, error = %e, "Failed to load categories");
            if let Ok(conn) = state.db.get() {
                let _ = ai_categorization::update_session_status(
                    &conn,
                    &session_id,
                    AiCategorizationStatus::Failed,
                );
                let _ = ai_categorization::update_session_errors(
                    &conn,
                    &session_id,
                    &[format!("Failed to load categories: {}", e)],
                );
            }
            return;
        }
    };

    let category_options: Vec<CategoryOption> = categories_with_path
        .iter()
        .map(|c| CategoryOption {
            id: c.category.id,
            path: c.path.clone(),
        })
        .collect();

    if category_options.is_empty() {
        error!(session_id = %session_id, "No categories available");
        if let Ok(conn) = state.db.get() {
            let _ = ai_categorization::update_session_status(
                &conn,
                &session_id,
                AiCategorizationStatus::Failed,
            );
            let _ = ai_categorization::update_session_errors(
                &conn,
                &session_id,
                &["No categories available. Please create some categories first.".to_string()],
            );
        }
        return;
    }

    // Load transactions to process
    let txns = match state.db.get() {
        Ok(conn) => {
            if include_categorized {
                transactions::list_all_transactions(&conn)
            } else {
                transactions::list_uncategorized_transactions(&conn)
            }
        }
        Err(e) => {
            error!(session_id = %session_id, error = %e, "Failed to get database connection");
            return;
        }
    };

    let txns = match txns {
        Ok(t) => t,
        Err(e) => {
            error!(session_id = %session_id, error = %e, "Failed to load transactions");
            if let Ok(conn) = state.db.get() {
                let _ = ai_categorization::update_session_status(
                    &conn,
                    &session_id,
                    AiCategorizationStatus::Failed,
                );
            }
            return;
        }
    };

    info!(session_id = %session_id, transaction_count = txns.len(), "Processing transactions");

    let mut processed = 0i64;
    let mut categorized = 0i64;
    let mut skipped = 0i64;
    let mut errors = 0i64;
    let mut error_messages: Vec<String> = Vec::new();

    // Process in batches
    for batch in txns.chunks(BATCH_SIZE) {
        // Check if cancelled
        if let Ok(conn) = state.db.get() {
            if let Ok(session) = ai_categorization::get_session(&conn, &session_id) {
                if session.is_cancelled() {
                    info!(session_id = %session_id, "Categorization cancelled");
                    return;
                }
            }
        }

        let transactions_for_ai: Vec<TransactionForCategorization> = batch
            .iter()
            .map(|t| TransactionForCategorization {
                id: t.id,
                date: t.date.clone(),
                description: t.description.clone(),
                amount: crate::filters::format_money_neutral(t.amount_cents, &t.currency, "en-US"),
                currency: t.currency.clone(),
            })
            .collect();

        let start_time = std::time::Instant::now();

        match categorize_transactions(&ai_settings, transactions_for_ai.clone(), &category_options)
            .await
        {
            Ok(suggestions) => {
                let duration_ms = start_time.elapsed().as_millis() as i64;

                // Log the API call
                if let Ok(conn) = state.db.get() {
                    let _ = api_logs::insert_api_log(
                        &conn,
                        &NewApiLog {
                            api_name: format!("ai_{}", ai_settings.provider.as_str()),
                            action: "categorize_transactions".to_string(),
                            symbol: None,
                            request_params: format!(
                                "batch_size={}, model={}",
                                batch.len(),
                                ai_settings.model
                            ),
                            status: "success".to_string(),
                            response_summary: Some(format!(
                                "Got {} suggestions",
                                suggestions.len()
                            )),
                            response_details: None,
                            duration_ms: Some(duration_ms),
                        },
                    );
                }

                // Store results
                if let Ok(conn) = state.db.get() {
                    for txn in batch {
                        let suggestion = suggestions.iter().find(|s| s.transaction_id == txn.id);

                        match suggestion {
                            Some(s) => {
                                if s.category_id.is_some() {
                                    let _ = ai_categorization::insert_result(
                                        &conn,
                                        &session_id,
                                        txn.id,
                                        txn.category_id,
                                        s.category_id,
                                        Some(s.confidence),
                                        Some(&s.reasoning),
                                        AiResultStatus::Pending,
                                        None,
                                    );
                                    categorized += 1;
                                } else {
                                    let _ = ai_categorization::insert_result(
                                        &conn,
                                        &session_id,
                                        txn.id,
                                        txn.category_id,
                                        None,
                                        Some(s.confidence),
                                        Some(&s.reasoning),
                                        AiResultStatus::Skipped,
                                        None,
                                    );
                                    skipped += 1;
                                }
                            }
                            None => {
                                // No suggestion returned for this transaction
                                let _ = ai_categorization::insert_result(
                                    &conn,
                                    &session_id,
                                    txn.id,
                                    txn.category_id,
                                    None,
                                    None,
                                    None,
                                    AiResultStatus::Skipped,
                                    Some("No suggestion returned by AI"),
                                );
                                skipped += 1;
                            }
                        }
                        processed += 1;
                    }

                    // Update progress
                    let _ = ai_categorization::update_session_progress(
                        &conn,
                        &session_id,
                        processed,
                        categorized,
                        skipped,
                        errors,
                    );
                }
            }
            Err(e) => {
                let duration_ms = start_time.elapsed().as_millis() as i64;
                warn!(session_id = %session_id, error = %e, "AI categorization batch failed");

                error_messages.push(format!(
                    "Batch error (transactions {}-{}): {}",
                    batch.first().map(|t| t.id).unwrap_or(0),
                    batch.last().map(|t| t.id).unwrap_or(0),
                    e
                ));

                // Log the failed API call
                if let Ok(conn) = state.db.get() {
                    let _ = api_logs::insert_api_log(
                        &conn,
                        &NewApiLog {
                            api_name: format!("ai_{}", ai_settings.provider.as_str()),
                            action: "categorize_transactions".to_string(),
                            symbol: None,
                            request_params: format!(
                                "batch_size={}, model={}",
                                batch.len(),
                                ai_settings.model
                            ),
                            status: "error".to_string(),
                            response_summary: Some(e.to_string()),
                            response_details: None,
                            duration_ms: Some(duration_ms),
                        },
                    );

                    // Mark all transactions in batch as errors
                    for txn in batch {
                        let _ = ai_categorization::insert_result(
                            &conn,
                            &session_id,
                            txn.id,
                            txn.category_id,
                            None,
                            None,
                            None,
                            AiResultStatus::Error,
                            Some(&e.to_string()),
                        );
                        processed += 1;
                        errors += 1;
                    }

                    let _ = ai_categorization::update_session_progress(
                        &conn,
                        &session_id,
                        processed,
                        categorized,
                        skipped,
                        errors,
                    );
                }
            }
        }

        // Rate limiting delay
        sleep(Duration::from_millis(RATE_LIMIT_DELAY_MS)).await;
    }

    // Finalize session
    if let Ok(conn) = state.db.get() {
        if !error_messages.is_empty() {
            let _ = ai_categorization::update_session_errors(&conn, &session_id, &error_messages);
        }
        let _ = ai_categorization::update_session_status(
            &conn,
            &session_id,
            AiCategorizationStatus::Completed,
        );
    }

    info!(
        session_id = %session_id,
        processed = processed,
        categorized = categorized,
        skipped = skipped,
        errors = errors,
        "AI categorization completed"
    );
}

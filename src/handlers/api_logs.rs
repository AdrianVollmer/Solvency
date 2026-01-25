use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, Json};
use serde::{Deserialize, Serialize};

use crate::db::queries::{api_logs, settings};
use crate::error::AppResult;
use crate::models::{ApiLog, Settings};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/api_logs.html")]
pub struct ApiLogsTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub logs: Vec<ApiLog>,
    pub latest_log_id: i64,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = settings::get_settings(&conn)?;
    let logs = api_logs::get_all_logs(&conn, 100)?;
    let latest_log_id = api_logs::get_latest_log_id(&conn)?;

    let template = ApiLogsTemplate {
        title: "API Logs".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        logs,
        latest_log_id,
    };

    Ok(Html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/api_log_detail.html")]
pub struct ApiLogDetailTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub log: ApiLog,
}

pub async fn detail(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = settings::get_settings(&conn)?;
    let log = api_logs::get_log_by_id(&conn, id)?
        .ok_or_else(|| crate::error::AppError::NotFound("API log not found".into()))?;

    let template = ApiLogDetailTemplate {
        title: format!("API Log #{}", id),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        log,
    };

    Ok(Html(template.render().unwrap()))
}

#[derive(Deserialize)]
pub struct PollQuery {
    since_id: i64,
}

#[derive(Serialize)]
pub struct PollResponse {
    pub new_errors: Vec<ApiLogSummary>,
    pub latest_id: i64,
}

#[derive(Serialize)]
pub struct ApiLogSummary {
    pub id: i64,
    pub symbol: Option<String>,
    pub action: String,
    pub error_message: String,
}

pub async fn poll_errors(
    State(state): State<AppState>,
    Query(query): Query<PollQuery>,
) -> AppResult<Json<PollResponse>> {
    let conn = state.db.get()?;
    let failed_logs = api_logs::get_failed_logs_since(&conn, query.since_id)?;
    let latest_id = api_logs::get_latest_log_id(&conn)?;

    let new_errors: Vec<ApiLogSummary> = failed_logs
        .into_iter()
        .map(|log| ApiLogSummary {
            id: log.id,
            symbol: log.symbol,
            action: log.action,
            error_message: log
                .response_summary
                .unwrap_or_else(|| "Unknown error".into()),
        })
        .collect();

    Ok(Json(PollResponse {
        new_errors,
        latest_id,
    }))
}

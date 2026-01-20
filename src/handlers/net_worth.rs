use askama::Template;
use axum::extract::State;
use axum::response::Html;
use axum::Json;
use serde::Serialize;

use crate::db::queries::settings;
use crate::error::AppResult;
use crate::filters;
use crate::models::net_worth::NetWorthDataPoint;
use crate::models::Settings;
use crate::services::net_worth::{calculate_net_worth_history, decimate_for_display};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

const MAX_CHART_POINTS: usize = 500;

#[derive(Template)]
#[template(path = "pages/net_worth.html")]
pub struct NetWorthTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub has_data: bool,
    pub current_net_worth_formatted: String,
    pub highest_net_worth_formatted: String,
    pub lowest_net_worth_formatted: String,
    pub starting_net_worth_formatted: String,
    pub change_formatted: String,
    pub change_percent_formatted: String,
    pub total_days: usize,
    pub start_date: String,
    pub end_date: String,
    pub current_net_worth_cents: i64,
    pub change_cents: i64,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = settings::get_settings(&conn)?;

    let summary = calculate_net_worth_history(&conn)?;

    let currency = &app_settings.currency;
    let locale = &app_settings.locale;

    let has_data = !summary.data_points.is_empty();
    let total_days = summary.data_points.len();

    // Get starting net worth (first data point)
    let starting_net_worth_cents = summary
        .data_points
        .first()
        .map(|p| p.net_worth_cents)
        .unwrap_or(0);

    // Calculate change from start
    let change_cents = summary.current_net_worth_cents - starting_net_worth_cents;
    let change_percent = if starting_net_worth_cents != 0 {
        (change_cents as f64 / starting_net_worth_cents.abs() as f64) * 100.0
    } else {
        0.0
    };

    let current_net_worth_formatted =
        filters::format_money_neutral(summary.current_net_worth_cents, currency, locale);
    let highest_net_worth_formatted =
        filters::format_money_neutral(summary.highest_net_worth_cents, currency, locale);
    let lowest_net_worth_formatted =
        filters::format_money_plain(summary.lowest_net_worth_cents, currency, locale);
    let starting_net_worth_formatted =
        filters::format_money_neutral(starting_net_worth_cents, currency, locale);
    let change_formatted = filters::format_money_plain(change_cents, currency, locale);
    let change_percent_formatted = format!(
        "{}{:.2}%",
        if change_percent >= 0.0 { "+" } else { "" },
        change_percent
    );

    let template = NetWorthTemplate {
        title: "Net Worth".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        version: VERSION,
        has_data,
        current_net_worth_formatted,
        highest_net_worth_formatted,
        lowest_net_worth_formatted,
        starting_net_worth_formatted,
        change_formatted,
        change_percent_formatted,
        total_days,
        start_date: summary.start_date,
        end_date: summary.end_date,
        current_net_worth_cents: summary.current_net_worth_cents,
        change_cents,
    };

    Ok(Html(template.render().unwrap()))
}

/// Chart data response
#[derive(Serialize)]
pub struct NetWorthChartResponse {
    pub labels: Vec<String>,
    pub net_worth: Vec<i64>,
    pub expense_component: Vec<i64>,
    pub portfolio_component: Vec<i64>,
}

impl NetWorthChartResponse {
    fn from_data_points(data_points: Vec<NetWorthDataPoint>) -> Self {
        let decimated = decimate_for_display(&data_points, MAX_CHART_POINTS);

        // Cap values to JavaScript's safe integer range to prevent precision loss
        const MAX_SAFE: i64 = 9_007_199_254_740_991; // 2^53 - 1
        let clamp = |v: i64| v.clamp(-MAX_SAFE, MAX_SAFE);

        Self {
            labels: decimated.iter().map(|p| p.date.clone()).collect(),
            net_worth: decimated.iter().map(|p| clamp(p.net_worth_cents)).collect(),
            expense_component: decimated
                .iter()
                .map(|p| clamp(p.expense_component_cents))
                .collect(),
            portfolio_component: decimated
                .iter()
                .map(|p| clamp(p.portfolio_component_cents))
                .collect(),
        }
    }
}

pub async fn chart_data(State(state): State<AppState>) -> AppResult<Json<NetWorthChartResponse>> {
    let conn = state.db.get()?;
    let summary = calculate_net_worth_history(&conn)?;

    let response = NetWorthChartResponse::from_data_points(summary.data_points);

    Ok(Json(response))
}

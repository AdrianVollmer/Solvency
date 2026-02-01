use askama::Template;
use axum::extract::{Query, State};
use axum::response::Html;
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;

use crate::date_utils::{DatePreset, DateRange};
use crate::db::queries::{categories, transactions};
use crate::error::{AppResult, RenderHtml};
use crate::handlers::transactions::TransactionPreviewTemplate;
use crate::models::category::CategoryWithPath;
use crate::models::Settings;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Debug, Default, Deserialize)]
pub struct SpendingFilterParams {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub preset: Option<String>,
    pub tab: Option<String>,
}

impl SpendingFilterParams {
    pub fn resolve_date_range(&self) -> DateRange {
        if let Some(preset_str) = &self.preset {
            preset_str
                .parse::<DatePreset>()
                .map(DateRange::from_preset)
                .unwrap_or_default()
        } else if let (Some(from), Some(to)) = (&self.from_date, &self.to_date) {
            if let (Ok(from_date), Ok(to_date)) = (
                NaiveDate::parse_from_str(from, "%Y-%m-%d"),
                NaiveDate::parse_from_str(to, "%Y-%m-%d"),
            ) {
                DateRange::from_dates(from_date, to_date)
            } else {
                DateRange::default()
            }
        } else {
            DateRange::default()
        }
    }
}

#[derive(Template)]
#[template(path = "pages/spending.html")]
pub struct SpendingTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub date_range: DateRange,
    pub presets: &'static [DatePreset],
    pub active_tab: String,
    pub base_qs: String,
    pub categories: Vec<CategoryWithPath>,
}

pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<SpendingFilterParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = state.load_settings()?;

    let date_range = params
        .resolve_date_range()
        .resolve_all(transactions::date_extent(&conn)?);

    let active_tab = match params.tab.as_deref() {
        Some("time") => "time".to_string(),
        Some("monthly") => "monthly".to_string(),
        Some("flow") => "flow".to_string(),
        _ => "category".to_string(),
    };

    let cats = categories::list_categories_with_path(&conn)?;

    let base_qs = format!("tab={}", active_tab);

    let template = SpendingTemplate {
        title: "Spending".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        date_range,
        presets: DatePreset::all(),
        active_tab,
        base_qs,
        categories: cats,
    };

    template.render_html()
}

#[derive(Debug, Deserialize)]
pub struct MonthlyTransactionsParams {
    pub month: String,
    pub category_ids: Option<String>,
}

pub async fn monthly_transactions(
    State(state): State<AppState>,
    Query(params): Query<MonthlyTransactionsParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    // Parse "2024-01" or "Jan 2024" style month strings
    let (from_date, to_date, month_label) = parse_month_range(&params.month)?;

    let category_ids: Vec<i64> = params
        .category_ids
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let filter = transactions::TransactionFilter {
        from_date: Some(from_date.clone()),
        to_date: Some(to_date.clone()),
        category_ids,
        sort_sql: Some("ABS(e.amount_cents) DESC".to_string()),
        limit: Some(20),
        ..Default::default()
    };

    let total_count = transactions::count_transactions(&conn, &filter)?;
    let transaction_list = transactions::list_transactions(&conn, &filter)?;

    let view_all_url = format!("/transactions?from_date={}&to_date={}", from_date, to_date);

    let template = TransactionPreviewTemplate {
        settings: app_settings,
        icons: crate::filters::Icons,
        title: "Monthly Transactions".to_string(),
        subtitle: month_label,
        transactions: transaction_list,
        count: total_count as usize,
        view_all_url,
    };

    template.render_html()
}

/// Parse a month string into (from_date, to_date, display_label).
/// Accepts "2024-01" (ISO) or "Jan 2024" / "January 2024" (chart label) formats.
fn parse_month_range(month: &str) -> AppResult<(String, String, String)> {
    let first_day = if let Ok(d) = NaiveDate::parse_from_str(&format!("{}-01", month), "%Y-%m-%d") {
        d
    } else {
        // Try "Jan 2024" or "January 2024" format from chart x-axis labels
        let formats = ["%b %Y", "%B %Y"];
        let mut parsed = None;
        for fmt in &formats {
            if let Ok(d) = NaiveDate::parse_from_str(month, fmt) {
                parsed = Some(d);
                break;
            }
        }
        parsed.ok_or_else(|| {
            crate::error::AppError::Validation(format!("Invalid month format: {}", month))
        })?
    };

    let last_day = if first_day.month() == 12 {
        NaiveDate::from_ymd_opt(first_day.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(first_day.year(), first_day.month() + 1, 1)
    }
    .expect("valid date")
    .pred_opt()
    .expect("valid date");

    let month_label = first_day.format("%B %Y").to_string();

    Ok((
        first_day.format("%Y-%m-%d").to_string(),
        last_day.format("%Y-%m-%d").to_string(),
        month_label,
    ))
}

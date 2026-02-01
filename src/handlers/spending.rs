use askama::Template;
use axum::extract::{Query, State};
use axum::response::Html;
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;

use crate::date_utils::{DatePreset, DateRange};
use crate::db::queries::transactions;
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

    let cats = state.cached_categories_with_path()?;

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

#[derive(Debug, Deserialize)]
pub struct CategoryTransactionsParams {
    pub category_id: Option<i64>,
    pub include_children: Option<bool>,
    pub uncategorized: Option<bool>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
}

pub async fn category_transactions(
    State(state): State<AppState>,
    Query(params): Query<CategoryTransactionsParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    let mut filter = transactions::TransactionFilter {
        from_date: params.from_date.clone(),
        to_date: params.to_date.clone(),
        sort_sql: Some("ABS(e.amount_cents) DESC".to_string()),
        limit: Some(20),
        ..Default::default()
    };

    let (title, subtitle) = if params.uncategorized == Some(true) {
        filter.uncategorized_only = true;
        ("Category Transactions".to_string(), "Uncategorized".to_string())
    } else if let Some(cat_id) = params.category_id {
        let categories = state.cached_categories()?;
        let cat_name = categories
            .iter()
            .find(|c| c.id == cat_id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("Category {}", cat_id));

        if params.include_children == Some(true) {
            let ids = collect_descendant_ids(&categories, cat_id);
            filter.category_ids = ids;
        } else {
            filter.category_ids = vec![cat_id];
        }

        ("Category Transactions".to_string(), cat_name)
    } else {
        return Err(crate::error::AppError::Validation(
            "category_id or uncategorized required".to_string(),
        ));
    };

    let total_count = transactions::count_transactions(&conn, &filter)?;
    let transaction_list = transactions::list_transactions(&conn, &filter)?;

    let mut view_all_url = "/transactions?".to_string();
    if let Some(cat_id) = params.category_id {
        view_all_url.push_str(&format!("category_id={}", cat_id));
    }
    if let Some(ref from) = params.from_date {
        view_all_url.push_str(&format!("&from_date={}", from));
    }
    if let Some(ref to) = params.to_date {
        view_all_url.push_str(&format!("&to_date={}", to));
    }

    let template = TransactionPreviewTemplate {
        settings: app_settings,
        icons: crate::filters::Icons,
        title,
        subtitle,
        transactions: transaction_list,
        count: total_count as usize,
        view_all_url,
    };

    template.render_html()
}

/// Collect a category and all its descendants from a flat list.
fn collect_descendant_ids(categories: &[crate::models::category::Category], parent_id: i64) -> Vec<i64> {
    let mut result = vec![parent_id];
    let mut i = 0;
    while i < result.len() {
        let current = result[i];
        for cat in categories {
            if cat.parent_id == Some(current) && !result.contains(&cat.id) {
                result.push(cat.id);
            }
        }
        i += 1;
    }
    result
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

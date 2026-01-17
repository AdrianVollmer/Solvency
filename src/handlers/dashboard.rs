use askama::Template;
use axum::extract::State;
use axum::response::Html;

use crate::db::queries::{expenses, settings};
use crate::error::AppResult;
use crate::models::{ExpenseWithRelations, Settings};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/dashboard.html")]
pub struct DashboardTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub recent_expenses: Vec<ExpenseWithRelations>,
    pub total_this_month: i64,
    pub total_last_month: i64,
    pub expense_count: i64,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let settings = settings::get_settings(&conn)?;

    let now = chrono::Local::now();
    let this_month_start = now.format("%Y-%m-01").to_string();
    let last_month = now - chrono::Duration::days(30);
    let last_month_start = last_month.format("%Y-%m-01").to_string();
    let last_month_end = now.format("%Y-%m-01").to_string();

    let filter = expenses::ExpenseFilter {
        limit: Some(5),
        ..Default::default()
    };
    let recent_expenses = expenses::list_expenses(&conn, &filter)?;

    let this_month_filter = expenses::ExpenseFilter {
        from_date: Some(this_month_start),
        ..Default::default()
    };
    let this_month_expenses = expenses::list_expenses(&conn, &this_month_filter)?;
    let total_this_month: i64 = this_month_expenses
        .iter()
        .map(|e| e.expense.amount_cents)
        .sum();

    let last_month_filter = expenses::ExpenseFilter {
        from_date: Some(last_month_start),
        to_date: Some(last_month_end),
        ..Default::default()
    };
    let last_month_expenses = expenses::list_expenses(&conn, &last_month_filter)?;
    let total_last_month: i64 = last_month_expenses
        .iter()
        .map(|e| e.expense.amount_cents)
        .sum();

    let expense_count = expenses::count_expenses(&conn, &expenses::ExpenseFilter::default())?;

    let template = DashboardTemplate {
        title: "Dashboard".into(),
        settings,
        manifest: state.manifest.clone(),
        version: VERSION,
        recent_expenses,
        total_this_month,
        total_last_month,
        expense_count,
    };

    Ok(Html(template.render().unwrap()))
}

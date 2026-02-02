use askama::Template;
use axum::extract::State;
use axum::response::Html;
use tracing::debug;

use crate::db::queries::transactions;
use crate::error::{AppResult, RenderHtml};
use crate::models::{Settings, TransactionWithRelations};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/dashboard.html")]
pub struct DashboardTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub recent_transactions: Vec<TransactionWithRelations>,
    pub total_this_month: i64,
    pub total_last_month: i64,
    pub transaction_count: i64,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    debug!("Loading dashboard");
    let conn = state.db.get()?;

    let settings = state.load_settings()?;

    let now = chrono::Local::now();
    let this_month_start = now.format("%Y-%m-01").to_string();
    let last_month = now - chrono::Duration::days(30);
    let last_month_start = last_month.format("%Y-%m-01").to_string();
    let last_month_end = now.format("%Y-%m-01").to_string();

    let filter = transactions::TransactionFilter {
        limit: Some(5),
        ..Default::default()
    };
    let recent_transactions = transactions::list_transactions(&conn, &filter)?;

    let total_this_month = transactions::sum_amount_cents(&conn, Some(&this_month_start), None)?;
    let total_last_month =
        transactions::sum_amount_cents(&conn, Some(&last_month_start), Some(&last_month_end))?;

    let transaction_count =
        transactions::count_transactions(&conn, &transactions::TransactionFilter::default())?;

    debug!(
        transaction_count = transaction_count,
        total_this_month = total_this_month,
        total_last_month = total_last_month,
        "Dashboard data loaded"
    );

    let template = DashboardTemplate {
        title: "Dashboard".into(),
        settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        recent_transactions,
        total_this_month,
        total_last_month,
        transaction_count,
    };

    template.render_html()
}

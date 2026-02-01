use askama::Template;
use axum::extract::{Query, State};
use axum::response::Html;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db::queries::{balances, market_data, trading, transactions};
use crate::error::{AppResult, RenderHtml};
use crate::filters;
use crate::handlers::transactions::TransactionPreviewTemplate;
use crate::models::account::AccountType;
use crate::models::net_worth::NetWorthDataPoint;
use crate::models::trading::PositionWithMarketData;
use crate::models::Settings;
use crate::services::net_worth::{calculate_net_worth_history, decimate_for_display};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

const MAX_CHART_POINTS: usize = 500;

const PALETTE: &[&str] = &[
    "#3b82f6", "#22c55e", "#f59e0b", "#ef4444", "#8b5cf6", "#06b6d4", "#f97316", "#ec4899",
    "#14b8a6", "#6366f1",
];

#[derive(Debug, Default, Deserialize)]
pub struct NetWorthParams {
    pub tab: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/net_worth.html")]
pub struct NetWorthTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
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
    pub active_tab: String,
}

pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<NetWorthParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

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
        filters::format_money_neutral(summary.lowest_net_worth_cents, currency, locale);
    let starting_net_worth_formatted =
        filters::format_money_neutral(starting_net_worth_cents, currency, locale);
    let change_formatted = filters::format_money_plain(change_cents, currency, locale);
    let change_percent_formatted = format!(
        "{}{:.2}%",
        if change_percent >= 0.0 { "+" } else { "" },
        change_percent
    );

    let active_tab = match params.tab.as_deref() {
        Some("allocation") => "allocation".to_string(),
        _ => "overview".to_string(),
    };

    let template = NetWorthTemplate {
        title: "Net Worth".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
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
        active_tab,
    };

    template.render_html()
}

/// Chart data response
#[derive(Serialize)]
pub struct NetWorthChartResponse {
    pub labels: Vec<String>,
    pub net_worth: Vec<i64>,
    pub transaction_component: Vec<i64>,
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
            transaction_component: decimated
                .iter()
                .map(|p| clamp(p.transaction_component_cents))
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

/// Query params for top transactions endpoint
#[derive(Deserialize)]
pub struct TopTransactionsParams {
    pub from_date: String,
    pub to_date: String,
}

/// Get top transactions by absolute value in a date range (returns HTML partial)
pub async fn top_transactions(
    State(state): State<AppState>,
    Query(params): Query<TopTransactionsParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    let filter = transactions::TransactionFilter {
        from_date: Some(params.from_date.clone()),
        to_date: Some(params.to_date.clone()),
        sort_sql: Some("ABS(e.amount_cents) DESC".to_string()),
        limit: Some(20),
        ..Default::default()
    };

    let total_count = transactions::count_transactions(&conn, &filter)?;
    let transaction_list = transactions::list_transactions(&conn, &filter)?;

    let view_all_url = format!(
        "/transactions?from_date={}&to_date={}",
        params.from_date, params.to_date
    );

    let template = TransactionPreviewTemplate {
        settings: app_settings,
        icons: crate::filters::Icons,
        title: "Largest Transactions".to_string(),
        subtitle: format!("{} to {}", params.from_date, params.to_date),
        transactions: transaction_list,
        count: total_count as usize,
        view_all_url,
    };

    template.render_html()
}

/// A node in the account allocation tree for the sunburst chart.
#[derive(Serialize)]
pub struct AllocationNode {
    pub name: String,
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_cents: Option<i64>,
    pub children: Vec<AllocationNode>,
}

/// Returns the account allocation tree for the sunburst chart.
/// Cash accounts are leaf nodes; securities accounts have children for each position.
pub async fn account_allocation(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AllocationNode>>> {
    let conn = state.db.get()?;

    let all_accounts = state.cached_accounts()?;
    let cash_balances = balances::get_cash_account_balances(&conn)?;

    let mut nodes: Vec<AllocationNode> = Vec::new();

    for (i, account) in all_accounts.iter().enumerate() {
        let color = PALETTE[i % PALETTE.len()].to_string();

        match account.account_type {
            AccountType::Cash => {
                let balance = *cash_balances.get(&account.id).unwrap_or(&0);
                if balance > 0 {
                    nodes.push(AllocationNode {
                        name: account.name.clone(),
                        color,
                        amount_cents: Some(balance),
                        children: vec![],
                    });
                }
            }
            AccountType::Securities => {
                let positions = trading::get_positions_for_account(&conn, account.id)?;
                let mut children: Vec<AllocationNode> = Vec::new();

                for pos in &positions {
                    let enriched =
                        if let Ok(Some(data)) = market_data::get_latest_price(&conn, &pos.symbol) {
                            PositionWithMarketData::with_market_data(
                                pos.clone(),
                                data.close_price_cents,
                                data.date,
                            )
                        } else if let Ok(Some((price_cents, date))) =
                            trading::get_last_trade_price(&conn, &pos.symbol)
                        {
                            PositionWithMarketData::with_approximated_price(
                                pos.clone(),
                                price_cents,
                                date,
                            )
                        } else {
                            PositionWithMarketData::from_position(pos.clone())
                        };

                    let value = enriched
                        .current_value_cents
                        .unwrap_or(enriched.position.total_cost_cents);
                    if value > 0 {
                        children.push(AllocationNode {
                            name: pos.symbol.clone(),
                            color: color.clone(),
                            amount_cents: Some(value),
                            children: vec![],
                        });
                    }
                }

                if !children.is_empty() {
                    nodes.push(AllocationNode {
                        name: account.name.clone(),
                        color,
                        amount_cents: None,
                        children,
                    });
                }
            }
        }
    }

    Ok(Json(nodes))
}

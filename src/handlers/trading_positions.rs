use askama::Template;
use axum::extract::State;
use axum::response::Html;

use crate::db::queries::{market_data, settings, trading};
use crate::error::AppResult;
use crate::models::trading::PositionWithMarketData;
use crate::models::{Position, Settings};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/trading_positions.html")]
pub struct TradingPositionsTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub positions: Vec<Position>,
    pub cash_positions: Vec<Position>,
    pub security_positions: Vec<PositionWithMarketData>,
    pub total_current_value: Option<i64>,
    pub total_current_value_formatted: Option<String>,
    pub total_cost: i64,
    pub total_cost_formatted: String,
    pub total_gain_loss: Option<i64>,
    pub total_gain_loss_color: &'static str,
    pub total_gain_loss_formatted: Option<String>,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

    let all_positions = trading::get_positions(&conn)?;

    // Separate cash and security positions
    let (cash_positions, security_only): (Vec<_>, Vec<_>) =
        all_positions.iter().cloned().partition(|p| p.is_cash());

    // Enrich security positions with market data
    let security_positions: Vec<PositionWithMarketData> = security_only
        .into_iter()
        .map(
            |pos| match market_data::get_latest_price(&conn, &pos.symbol) {
                Ok(Some(data)) => {
                    PositionWithMarketData::with_market_data(pos, data.close_price_cents, data.date)
                }
                _ => PositionWithMarketData::from_position(pos),
            },
        )
        .collect();

    // Calculate totals
    let total_cost: i64 = security_positions
        .iter()
        .map(|p| p.position.total_cost_cents)
        .sum();

    let total_current_value: Option<i64> = {
        let values: Vec<_> = security_positions
            .iter()
            .filter_map(|p| p.current_value_cents)
            .collect();
        if !values.is_empty() {
            Some(values.iter().sum())
        } else {
            None
        }
    };

    let total_gain_loss = total_current_value.map(|cv| cv - total_cost);

    // Compute gain/loss display values
    let total_gain_loss_color = match total_gain_loss {
        Some(gl) if gl > 0 => "text-green-600 dark:text-green-400",
        Some(gl) if gl < 0 => "text-red-600 dark:text-red-400",
        _ => "text-neutral-600 dark:text-neutral-400",
    };

    let total_gain_loss_formatted = total_gain_loss.map(|gl| {
        let sign = if gl < 0 { "-" } else { "" };
        let dollars = gl.abs() / 100;
        let cents = gl.abs() % 100;
        format!("{}${}.{:02}", sign, dollars, cents)
    });

    let total_cost_formatted = {
        let dollars = total_cost / 100;
        let cents = total_cost.abs() % 100;
        format!("${}.{:02}", dollars, cents)
    };

    let total_current_value_formatted = total_current_value.map(|val| {
        let dollars = val / 100;
        let cents = val.abs() % 100;
        format!("${}.{:02}", dollars, cents)
    });

    let template = TradingPositionsTemplate {
        title: "Positions".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        version: VERSION,
        positions: all_positions,
        cash_positions,
        security_positions,
        total_current_value,
        total_current_value_formatted,
        total_cost,
        total_cost_formatted,
        total_gain_loss,
        total_gain_loss_color,
        total_gain_loss_formatted,
    };

    Ok(Html(template.render().unwrap()))
}

use askama::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::Json;
use chrono::NaiveDate;
use serde::Serialize;

use crate::db::queries::{market_data, settings, trading};
use crate::error::AppResult;
use crate::models::trading::{PositionWithMarketData, TradingActivity, TradingActivityType};
use crate::models::{MarketData, Position, Settings};
use crate::services::market_data as market_data_service;
use crate::services::xirr::{calculate_xirr, CashFlow};
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
        .map(|pos| {
            // First try to get actual market data
            if let Ok(Some(data)) = market_data::get_latest_price(&conn, &pos.symbol) {
                return PositionWithMarketData::with_market_data(
                    pos,
                    data.close_price_cents,
                    data.date,
                );
            }
            // Fall back to last BUY/SELL price as approximation
            if let Ok(Some((price_cents, date))) = trading::get_last_trade_price(&conn, &pos.symbol)
            {
                return PositionWithMarketData::with_approximated_price(pos, price_cents, date);
            }
            // No price data available
            PositionWithMarketData::from_position(pos)
        })
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

// Position detail page

/// Symbol metadata for display
#[derive(Debug, Clone, Default)]
pub struct SymbolInfo {
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub exchange: Option<String>,
    pub quote_type: Option<String>,
}

impl SymbolInfo {
    pub fn display_name(&self) -> Option<&String> {
        self.long_name.as_ref().or(self.short_name.as_ref())
    }
}

#[derive(Template)]
#[template(path = "pages/position_detail.html")]
pub struct PositionDetailTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub symbol: String,
    pub symbol_info: SymbolInfo,
    pub position: Option<PositionWithMarketData>,
    pub activities: Vec<TradingActivity>,
    pub xirr: Option<f64>,
    pub xirr_formatted: Option<String>,
    pub latest_price: Option<MarketData>,
}

pub async fn detail(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

    // Fetch symbol metadata from Yahoo Finance
    let symbol_info = match market_data_service::fetch_symbol_metadata(&symbol).await {
        Ok(Some(meta)) => SymbolInfo {
            short_name: meta.short_name,
            long_name: meta.long_name,
            exchange: Some(meta.exchange),
            quote_type: Some(meta.quote_type),
        },
        _ => SymbolInfo::default(),
    };

    // Get all positions and find the one for this symbol
    let all_positions = trading::get_positions(&conn)?;
    let position_opt = all_positions.into_iter().find(|p| p.symbol == symbol);

    // Enrich with market data if position exists
    let position =
        position_opt.map(
            |pos| match market_data::get_latest_price(&conn, &pos.symbol) {
                Ok(Some(data)) => {
                    PositionWithMarketData::with_market_data(pos, data.close_price_cents, data.date)
                }
                _ => PositionWithMarketData::from_position(pos),
            },
        );

    // Get activities for this symbol
    let activities = trading::get_activities_for_symbol(&conn, &symbol)?;

    // Get latest price
    let latest_price = market_data::get_latest_price(&conn, &symbol)?;

    // Calculate XIRR
    let xirr = calculate_position_xirr(&activities, &position, &latest_price);
    let xirr_formatted = xirr.map(|x| format!("{:+.2}%", x * 100.0));

    let display_name = symbol_info
        .display_name()
        .map(|n| format!("{} ({})", n, symbol))
        .unwrap_or_else(|| symbol.clone());

    let template = PositionDetailTemplate {
        title: display_name,
        settings: app_settings,
        manifest: state.manifest.clone(),
        version: VERSION,
        symbol: symbol.clone(),
        symbol_info,
        position,
        activities,
        xirr,
        xirr_formatted,
        latest_price,
    };

    Ok(Html(template.render().unwrap()))
}

/// Calculate XIRR for a position based on its activities
fn calculate_position_xirr(
    activities: &[TradingActivity],
    position: &Option<PositionWithMarketData>,
    latest_price: &Option<MarketData>,
) -> Option<f64> {
    let mut cash_flows: Vec<CashFlow> = Vec::new();

    for activity in activities {
        let date = match NaiveDate::parse_from_str(&activity.date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue, // Skip activities with invalid dates
        };

        let amount = match activity.activity_type {
            // Buys are cash outflows (negative)
            TradingActivityType::Buy | TradingActivityType::AddHolding => {
                let qty = activity.quantity.unwrap_or(0.0);
                let price = activity.unit_price_cents.unwrap_or(0) as f64 / 100.0;
                let fee = activity.fee_cents as f64 / 100.0;
                -(qty * price + fee)
            }
            // Sells are cash inflows (positive)
            TradingActivityType::Sell | TradingActivityType::RemoveHolding => {
                let qty = activity.quantity.unwrap_or(0.0);
                let price = activity.unit_price_cents.unwrap_or(0) as f64 / 100.0;
                let fee = activity.fee_cents as f64 / 100.0;
                qty * price - fee
            }
            // Dividends are cash inflows (positive)
            TradingActivityType::Dividend => {
                let qty = activity.quantity.unwrap_or(0.0);
                let price = activity.unit_price_cents.unwrap_or(0) as f64 / 100.0;
                qty * price
            }
            // Other activity types don't affect XIRR calculation for securities
            _ => continue,
        };

        if amount.abs() > 0.001 {
            cash_flows.push(CashFlow { date, amount });
        }
    }

    // Add current position value as final cash flow (as if selling today)
    if let (Some(pos), Some(price_data)) = (position, latest_price) {
        if pos.current_value_cents.is_some() {
            let current_value = pos.current_value_cents.unwrap() as f64 / 100.0;
            let date = NaiveDate::parse_from_str(&price_data.date, "%Y-%m-%d")
                .unwrap_or_else(|_| chrono::Local::now().date_naive());

            cash_flows.push(CashFlow {
                date,
                amount: current_value,
            });
        }
    }

    calculate_xirr(&cash_flows)
}

// Chart data API

#[derive(Serialize)]
pub struct PositionChartData {
    pub date: String,
    pub price_cents: i64,
}

#[derive(Serialize)]
pub struct ActivityMarker {
    pub date: String,
    pub activity_type: String,
    pub quantity: f64,
    pub price_cents: i64,
    pub total_cents: i64,
}

#[derive(Serialize)]
pub struct PositionChartResponse {
    pub symbol: String,
    pub data: Vec<PositionChartData>,
    pub activities: Vec<ActivityMarker>,
}

pub async fn position_chart_data(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<PositionChartResponse>> {
    let conn = state.db.get()?;

    // Get all price data for this symbol (ordered by date descending, we need ascending)
    let mut data_points = market_data::get_prices_for_symbol(&conn, &symbol)?;
    data_points.reverse(); // Now oldest first

    let data: Vec<PositionChartData> = data_points
        .into_iter()
        .map(|dp| PositionChartData {
            date: dp.date,
            price_cents: dp.close_price_cents,
        })
        .collect();

    // Get buy/sell activities for markers
    let all_activities = trading::get_activities_for_symbol(&conn, &symbol)?;
    let activities: Vec<ActivityMarker> = all_activities
        .into_iter()
        .filter(|a| {
            matches!(
                a.activity_type,
                TradingActivityType::Buy | TradingActivityType::Sell
            )
        })
        .filter_map(|a| {
            let qty = a.quantity?;
            let price = a.unit_price_cents?;
            Some(ActivityMarker {
                date: a.date,
                activity_type: a.activity_type.as_str().to_string(),
                quantity: qty,
                price_cents: price,
                total_cents: (qty * price as f64).round() as i64,
            })
        })
        .collect();

    Ok(Json(PositionChartResponse {
        symbol,
        data,
        activities,
    }))
}

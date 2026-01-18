use askama::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::Json;
use chrono::NaiveDate;
use serde::Serialize;

use crate::db::queries::{market_data, settings, trading};
use crate::error::AppResult;
use crate::models::trading::{
    ClosedPosition, PositionWithMarketData, TradingActivity, TradingActivityType,
};
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

// Closed positions page

#[derive(Template)]
#[template(path = "pages/trading_positions_closed.html")]
pub struct ClosedPositionsTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub positions: Vec<ClosedPosition>,
    pub total_cost: i64,
    pub total_cost_formatted: String,
    pub total_proceeds: i64,
    pub total_proceeds_formatted: String,
    pub total_gain_loss: i64,
    pub total_gain_loss_formatted: String,
    pub total_gain_loss_color: &'static str,
}

pub async fn closed_positions(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

    let positions = trading::get_closed_positions(&conn)?;

    // Calculate totals
    let total_cost: i64 = positions.iter().map(|p| p.total_cost_cents).sum();
    let total_proceeds: i64 = positions.iter().map(|p| p.total_proceeds_cents).sum();
    let total_gain_loss = total_proceeds - total_cost;

    let total_gain_loss_color = if total_gain_loss > 0 {
        "text-green-600 dark:text-green-400"
    } else if total_gain_loss < 0 {
        "text-red-600 dark:text-red-400"
    } else {
        "text-neutral-600 dark:text-neutral-400"
    };

    let format_cents = |cents: i64| {
        let sign = if cents < 0 { "-" } else { "" };
        let dollars = cents.abs() / 100;
        let remainder = cents.abs() % 100;
        format!("{}${}.{:02}", sign, dollars, remainder)
    };

    let template = ClosedPositionsTemplate {
        title: "Closed Positions".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        version: VERSION,
        positions,
        total_cost,
        total_cost_formatted: format_cents(total_cost),
        total_proceeds,
        total_proceeds_formatted: format_cents(total_proceeds),
        total_gain_loss,
        total_gain_loss_formatted: format_cents(total_gain_loss),
        total_gain_loss_color,
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
    pub total_fees_cents: i64,
    pub total_fees_formatted: String,
    pub total_taxes_cents: i64,
    pub total_taxes_formatted: String,
    pub total_dividends_cents: i64,
    pub total_dividends_formatted: String,
    pub realized_gain_loss_cents: i64,
    pub realized_gain_loss_formatted: String,
    pub realized_gain_loss_color: &'static str,
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

    // Calculate total fees, taxes, dividends, and realized gain/loss
    let (total_fees_cents, total_taxes_cents, total_dividends_cents, realized_gain_loss_cents) =
        calculate_position_totals(&activities);

    let currency = position
        .as_ref()
        .map(|p| p.position.currency.as_str())
        .unwrap_or("USD");

    let format_cents = |cents: i64| {
        let sign = if cents < 0 { "-" } else { "" };
        let symbol = match currency.to_uppercase().as_str() {
            "EUR" => "€",
            "GBP" => "£",
            _ => "$",
        };
        let dollars = cents.abs() / 100;
        let remainder = cents.abs() % 100;
        format!("{}{}{}.{:02}", sign, symbol, dollars, remainder)
    };

    let total_fees_formatted = format_cents(total_fees_cents);
    let total_taxes_formatted = format_cents(total_taxes_cents);
    let total_dividends_formatted = format_cents(total_dividends_cents);
    let realized_gain_loss_formatted = format_cents(realized_gain_loss_cents);

    let realized_gain_loss_color = if realized_gain_loss_cents > 0 {
        "text-green-600 dark:text-green-400"
    } else if realized_gain_loss_cents < 0 {
        "text-red-600 dark:text-red-400"
    } else {
        "text-neutral-600 dark:text-neutral-400"
    };

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
        total_fees_cents,
        total_fees_formatted,
        total_taxes_cents,
        total_taxes_formatted,
        total_dividends_cents,
        total_dividends_formatted,
        realized_gain_loss_cents,
        realized_gain_loss_formatted,
        realized_gain_loss_color,
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

/// Calculate total fees, taxes, dividends, and realized gain/loss for a position
/// Returns (total_fees_cents, total_taxes_cents, total_dividends_cents, realized_gain_loss_cents)
/// Note: Dividends are included in realized_gain_loss_cents
fn calculate_position_totals(activities: &[TradingActivity]) -> (i64, i64, i64, i64) {
    let mut total_fees_cents: i64 = 0;
    let mut total_taxes_cents: i64 = 0;
    let mut total_dividends_cents: i64 = 0;
    let mut realized_gain_loss_cents: i64 = 0;

    // Track running position for average cost calculation
    let mut running_quantity: f64 = 0.0;
    let mut running_cost_cents: i64 = 0;

    for activity in activities {
        // Fee activity type stores fee amount in unit_price_cents
        if activity.activity_type == TradingActivityType::Fee {
            if let Some(price) = activity.unit_price_cents {
                total_fees_cents += price;
            }
        }

        // Sum taxes (Tax activity type stores total tax amount in unit_price_cents)
        if activity.activity_type == TradingActivityType::Tax {
            if let Some(price) = activity.unit_price_cents {
                total_taxes_cents += price;
            }
        }

        // Sum dividends and include in realized gain/loss
        // Dividend activity type stores total dividend amount in unit_price_cents
        if activity.activity_type == TradingActivityType::Dividend {
            if let Some(price) = activity.unit_price_cents {
                total_dividends_cents += price;
                realized_gain_loss_cents += price;
            }
        }

        // Calculate realized gain/loss from sell activities
        match activity.activity_type {
            TradingActivityType::Buy | TradingActivityType::AddHolding => {
                let qty = activity.quantity.unwrap_or(0.0);
                let price = activity.unit_price_cents.unwrap_or(0);
                let cost = (qty * price as f64).round() as i64;
                running_quantity += qty;
                running_cost_cents += cost;
            }
            TradingActivityType::Sell | TradingActivityType::RemoveHolding => {
                let qty = activity.quantity.unwrap_or(0.0);
                let sell_price = activity.unit_price_cents.unwrap_or(0);
                let sell_value = (qty * sell_price as f64).round() as i64;

                // Calculate cost basis using average cost
                if running_quantity > 0.0 {
                    let avg_cost = running_cost_cents as f64 / running_quantity;
                    let cost_basis = (qty * avg_cost).round() as i64;

                    // Realized gain/loss = sell value - cost basis
                    realized_gain_loss_cents += sell_value - cost_basis;

                    // Update running position
                    running_quantity -= qty;
                    running_cost_cents -= cost_basis;

                    if running_quantity < 0.0 {
                        running_quantity = 0.0;
                    }
                    if running_cost_cents < 0 {
                        running_cost_cents = 0;
                    }
                }
            }
            TradingActivityType::TransferIn => {
                let qty = activity.quantity.unwrap_or(0.0);
                let price = activity.unit_price_cents.unwrap_or(0);
                let cost = (qty * price as f64).round() as i64;
                running_quantity += qty;
                running_cost_cents += cost;
            }
            TradingActivityType::TransferOut => {
                let qty = activity.quantity.unwrap_or(0.0);
                let sell_price = activity.unit_price_cents.unwrap_or(0);
                let sell_value = (qty * sell_price as f64).round() as i64;

                if running_quantity > 0.0 {
                    let avg_cost = running_cost_cents as f64 / running_quantity;
                    let cost_basis = (qty * avg_cost).round() as i64;

                    realized_gain_loss_cents += sell_value - cost_basis;

                    running_quantity -= qty;
                    running_cost_cents -= cost_basis;

                    if running_quantity < 0.0 {
                        running_quantity = 0.0;
                    }
                    if running_cost_cents < 0 {
                        running_cost_cents = 0;
                    }
                }
            }
            TradingActivityType::Split => {
                // Split adjusts quantity but not cost
                if let Some(ratio) = activity.quantity {
                    if ratio > 0.0 {
                        running_quantity *= ratio;
                    }
                }
            }
            _ => {}
        }
    }

    (
        total_fees_cents,
        total_taxes_cents,
        total_dividends_cents,
        realized_gain_loss_cents,
    )
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

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::Json;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::db::queries::{market_data, settings, trading};
use crate::error::AppResult;
use crate::filters;
use crate::models::trading::{
    ClosedPosition, PositionWithMarketData, TradingActivity, TradingActivityType,
};
use crate::models::{MarketData, Position, Settings};
use crate::services::xirr::{calculate_xirr, CashFlow};
use crate::sort_utils::{SortDirection, Sortable, SortableColumn, TableSort};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

/// Sortable columns for the positions table.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum PositionSortColumn {
    #[default]
    Symbol,
    Quantity,
    Price,
    AvgCost,
    TotalCost,
    Value,
    GainLoss,
}

/// Sortable columns for the closed positions table.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum ClosedPositionSortColumn {
    #[default]
    Symbol,
    TotalCost,
    Proceeds,
    GainLoss,
    Period,
}

impl SortableColumn for PositionSortColumn {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "symbol" => Some(Self::Symbol),
            "quantity" => Some(Self::Quantity),
            "price" => Some(Self::Price),
            "avgcost" => Some(Self::AvgCost),
            "totalcost" => Some(Self::TotalCost),
            "value" => Some(Self::Value),
            "gainloss" => Some(Self::GainLoss),
            _ => None,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Symbol => "symbol",
            Self::Quantity => "quantity",
            Self::Price => "price",
            Self::AvgCost => "avgcost",
            Self::TotalCost => "totalcost",
            Self::Value => "value",
            Self::GainLoss => "gainloss",
        }
    }

    fn sql_expression(&self) -> &'static str {
        // Not used for in-memory sorting
        ""
    }
}

impl SortableColumn for ClosedPositionSortColumn {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "symbol" => Some(Self::Symbol),
            "totalcost" => Some(Self::TotalCost),
            "proceeds" => Some(Self::Proceeds),
            "gainloss" => Some(Self::GainLoss),
            "period" => Some(Self::Period),
            _ => None,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Symbol => "symbol",
            Self::TotalCost => "totalcost",
            Self::Proceeds => "proceeds",
            Self::GainLoss => "gainloss",
            Self::Period => "period",
        }
    }

    fn sql_expression(&self) -> &'static str {
        // Not used for in-memory sorting
        ""
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PositionFilterParams {
    pub sort: Option<String>,
    pub dir: Option<String>,
}

impl Sortable for PositionFilterParams {
    fn sort_by(&self) -> Option<&String> {
        self.sort.as_ref()
    }

    fn sort_dir(&self) -> Option<&String> {
        self.dir.as_ref()
    }
}

/// Sort positions in-memory based on sort configuration.
fn sort_positions(positions: &mut [PositionWithMarketData], sort: &TableSort<PositionSortColumn>) {
    positions.sort_by(|a, b| {
        let cmp = match sort.column {
            PositionSortColumn::Symbol => a.position.symbol.cmp(&b.position.symbol),
            PositionSortColumn::Quantity => a
                .position
                .quantity
                .partial_cmp(&b.position.quantity)
                .unwrap_or(std::cmp::Ordering::Equal),
            PositionSortColumn::Price => a.current_price_cents.cmp(&b.current_price_cents),
            PositionSortColumn::AvgCost => a
                .position
                .average_cost_cents()
                .cmp(&b.position.average_cost_cents()),
            PositionSortColumn::TotalCost => a
                .position
                .total_cost_cents
                .cmp(&b.position.total_cost_cents),
            PositionSortColumn::Value => a.current_value_cents.cmp(&b.current_value_cents),
            PositionSortColumn::GainLoss => a.gain_loss_cents.cmp(&b.gain_loss_cents),
        };

        match sort.direction {
            SortDirection::Asc => cmp,
            SortDirection::Desc => cmp.reverse(),
        }
    });
}

/// Sort closed positions in-memory based on sort configuration.
fn sort_closed_positions(
    positions: &mut [ClosedPosition],
    sort: &TableSort<ClosedPositionSortColumn>,
) {
    positions.sort_by(|a, b| {
        let cmp = match sort.column {
            ClosedPositionSortColumn::Symbol => a.symbol.cmp(&b.symbol),
            ClosedPositionSortColumn::TotalCost => a.total_cost_cents.cmp(&b.total_cost_cents),
            ClosedPositionSortColumn::Proceeds => {
                a.total_proceeds_cents.cmp(&b.total_proceeds_cents)
            }
            ClosedPositionSortColumn::GainLoss => {
                a.realized_gain_loss_cents.cmp(&b.realized_gain_loss_cents)
            }
            ClosedPositionSortColumn::Period => a.first_activity_date.cmp(&b.first_activity_date),
        };

        match sort.direction {
            SortDirection::Asc => cmp,
            SortDirection::Desc => cmp.reverse(),
        }
    });
}

#[derive(Template)]
#[template(path = "pages/trading_positions.html")]
pub struct TradingPositionsTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
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
    pub sort: TableSort<PositionSortColumn>,
}

pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<PositionFilterParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;
    let sort: TableSort<PositionSortColumn> = params.resolve_sort();

    let all_positions = trading::get_positions(&conn)?;

    // Separate cash and security positions
    let (cash_positions, security_only): (Vec<_>, Vec<_>) =
        all_positions.iter().cloned().partition(|p| p.is_cash());

    // Enrich security positions with market data
    let mut security_positions: Vec<PositionWithMarketData> = security_only
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

    // Sort positions
    sort_positions(&mut security_positions, &sort);

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

    let currency = &app_settings.currency;
    let locale = &app_settings.locale;

    let total_gain_loss_formatted =
        total_gain_loss.map(|gl| filters::format_money_neutral(gl, currency, locale));

    let total_cost_formatted = filters::format_money_neutral(total_cost, currency, locale);

    let total_current_value_formatted =
        total_current_value.map(|val| filters::format_money_neutral(val, currency, locale));

    let template = TradingPositionsTemplate {
        title: "Positions".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
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
        sort,
    };

    Ok(Html(template.render().unwrap()))
}

// Closed positions page

#[derive(Template)]
#[template(path = "pages/trading_positions_closed.html")]
pub struct ClosedPositionsTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub positions: Vec<ClosedPosition>,
    pub total_cost: i64,
    pub total_cost_formatted: String,
    pub total_proceeds: i64,
    pub total_proceeds_formatted: String,
    pub total_gain_loss: i64,
    pub total_gain_loss_formatted: String,
    pub total_gain_loss_color: &'static str,
    pub sort: TableSort<ClosedPositionSortColumn>,
}

pub async fn closed_positions(
    State(state): State<AppState>,
    Query(params): Query<PositionFilterParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;
    let sort: TableSort<ClosedPositionSortColumn> = params.resolve_sort();

    let mut positions = trading::get_closed_positions(&conn)?;

    // Sort positions
    sort_closed_positions(&mut positions, &sort);

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

    let total_cost_formatted =
        filters::format_money_neutral(total_cost, &app_settings.currency, &app_settings.locale);
    let total_proceeds_formatted =
        filters::format_money_neutral(total_proceeds, &app_settings.currency, &app_settings.locale);
    let total_gain_loss_formatted = filters::format_money_neutral(
        total_gain_loss,
        &app_settings.currency,
        &app_settings.locale,
    );

    let template = ClosedPositionsTemplate {
        title: "Closed Positions".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        positions,
        total_cost,
        total_cost_formatted,
        total_proceeds,
        total_proceeds_formatted,
        total_gain_loss,
        total_gain_loss_formatted,
        total_gain_loss_color,
        sort,
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
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
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

    // Get cached symbol metadata from DB
    let symbol_info = match market_data::get_symbol_metadata(&conn, &symbol) {
        Ok(Some(meta)) => SymbolInfo {
            short_name: meta.short_name,
            long_name: meta.long_name,
            exchange: meta.exchange,
            quote_type: meta.quote_type,
        },
        _ => SymbolInfo::default(),
    };

    // Get all positions and find the one for this symbol
    let all_positions = trading::get_positions(&conn)?;
    let position_opt = all_positions.into_iter().find(|p| p.symbol == symbol);

    // Enrich with market data if position exists (same logic as positions list)
    let position = position_opt.map(|pos| {
        // First try to get actual market data
        if let Ok(Some(data)) = market_data::get_latest_price(&conn, &pos.symbol) {
            return PositionWithMarketData::with_market_data(
                pos,
                data.close_price_cents,
                data.date,
            );
        }
        // Fall back to last BUY/SELL price as approximation
        if let Ok(Some((price_cents, date))) = trading::get_last_trade_price(&conn, &pos.symbol) {
            return PositionWithMarketData::with_approximated_price(pos, price_cents, date);
        }
        // No price data available
        PositionWithMarketData::from_position(pos)
    });

    // Get activities for this symbol
    let activities = trading::get_activities_for_symbol(&conn, &symbol)?;

    // Get latest price
    let latest_price = market_data::get_latest_price(&conn, &symbol)?;

    // Calculate XIRR
    let xirr = calculate_position_xirr(&activities, &position, &latest_price);
    let xirr_formatted = xirr.map(|x| filters::format_percent(x * 100.0, &app_settings.locale));

    // Calculate total fees, taxes, dividends, and realized gain/loss
    let (total_fees_cents, total_taxes_cents, total_dividends_cents, realized_gain_loss_cents) =
        calculate_position_totals(&activities);

    let currency = position
        .as_ref()
        .map(|p| p.position.currency.as_str())
        .unwrap_or(&app_settings.currency);

    let locale = &app_settings.locale;

    let total_fees_formatted = filters::format_money_neutral(total_fees_cents, currency, locale);
    let total_taxes_formatted = filters::format_money_neutral(total_taxes_cents, currency, locale);
    let total_dividends_formatted =
        filters::format_money_neutral(total_dividends_cents, currency, locale);
    let realized_gain_loss_formatted =
        filters::format_money_plain(realized_gain_loss_cents, currency, locale);

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
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
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

    // Subtract fees and taxes from realized gain/loss
    let net_realized_gain_loss_cents =
        realized_gain_loss_cents - total_fees_cents - total_taxes_cents;

    (
        total_fees_cents,
        total_taxes_cents,
        total_dividends_cents,
        net_realized_gain_loss_cents,
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
    pub is_approximated: bool,
}

pub async fn position_chart_data(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<PositionChartResponse>> {
    let conn = state.db.get()?;

    // Get all price data for this symbol (ordered by date descending, we need ascending)
    let mut data_points = market_data::get_prices_for_symbol(&conn, &symbol)?;
    data_points.reverse(); // Now oldest first

    let (data, is_approximated): (Vec<PositionChartData>, bool) = if !data_points.is_empty() {
        // Use actual market data
        let chart_data = data_points
            .into_iter()
            .map(|dp| PositionChartData {
                date: dp.date,
                price_cents: dp.close_price_cents,
            })
            .collect();
        (chart_data, false)
    } else {
        // No market data available - build step function from trade prices
        let chart_data = trading::get_all_trade_prices(&conn, &symbol)
            .unwrap_or_default()
            .into_iter()
            .map(|(date, price_cents)| PositionChartData { date, price_cents })
            .collect();
        (chart_data, true)
    };

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
        is_approximated,
    }))
}

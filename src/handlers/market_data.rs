use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, Redirect};
use axum::Json;
use serde::Serialize;

use chrono::Datelike;

use crate::db::queries::{market_data, settings};
use crate::error::AppResult;
use crate::models::{MarketData, Settings, SymbolDataCoverage};
use crate::services::market_data as market_data_service;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/market_data.html")]
pub struct MarketDataTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub coverage: Vec<SymbolDataCoverage>,
    pub total_data_points: i64,
    pub symbols_needing_data: usize,
    pub is_refreshing: bool,
    pub refresh_message: Option<String>,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;
    let coverage = market_data::get_symbol_coverage(&conn)?;
    let total_data_points = market_data::count_market_data(&conn)?;
    let symbols_needing_data = market_data::get_symbols_needing_data(&conn)?.len();

    let template = MarketDataTemplate {
        title: "Market Data".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        version: VERSION,
        coverage,
        total_data_points,
        symbols_needing_data,
        is_refreshing: false,
        refresh_message: None,
    };

    Ok(Html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "partials/market_data_status.html")]
pub struct MarketDataStatusTemplate {
    pub coverage: Vec<SymbolDataCoverage>,
    pub total_data_points: i64,
    pub symbols_needing_data: usize,
    pub refresh_message: Option<String>,
}

pub async fn refresh(State(state): State<AppState>) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    // Get symbols that need data
    let symbols_to_fetch = market_data::get_symbols_needing_data(&conn)?;

    if symbols_to_fetch.is_empty() {
        return Ok(Redirect::to("/trading/market-data"));
    }

    // Spawn background task for fetching
    let state_clone = state.clone();
    tokio::spawn(async move {
        for (symbol, start_date, end_date) in symbols_to_fetch {
            match market_data_service::fetch_historical_quotes(&symbol, &start_date, &end_date)
                .await
            {
                Ok(data) => {
                    if let Ok(conn) = state_clone.db.get() {
                        if let Err(e) = market_data::insert_market_data_batch(&conn, &data) {
                            tracing::error!("Failed to insert market data for {}: {}", symbol, e);
                        } else {
                            tracing::info!("Fetched {} data points for {}", data.len(), symbol);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to fetch market data for {}: {}", symbol, e);
                }
            }

            // Rate limiting between symbols
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    });

    Ok(Redirect::to("/trading/market-data"))
}

pub async fn refresh_symbol(
    State(state): State<AppState>,
    axum::extract::Path(symbol): axum::extract::Path<String>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    // Get the date range for this symbol
    let symbols_needing = market_data::get_symbols_needing_data(&conn)?;
    let symbol_info = symbols_needing.iter().find(|(s, _, _)| s == &symbol);

    if let Some((_, start_date, end_date)) = symbol_info {
        let start = start_date.clone();
        let end = end_date.clone();
        let sym = symbol.clone();

        // Spawn background task
        let state_clone = state.clone();
        tokio::spawn(async move {
            match market_data_service::fetch_historical_quotes(&sym, &start, &end).await {
                Ok(data) => {
                    if let Ok(conn) = state_clone.db.get() {
                        if let Err(e) = market_data::insert_market_data_batch(&conn, &data) {
                            tracing::error!("Failed to insert market data for {}: {}", sym, e);
                        } else {
                            tracing::info!("Fetched {} data points for {}", data.len(), sym);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to fetch market data for {}: {}", sym, e);
                }
            }
        });
    }

    Ok(Redirect::to("/trading/market-data"))
}

pub async fn status(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let coverage = market_data::get_symbol_coverage(&conn)?;
    let total_data_points = market_data::count_market_data(&conn)?;
    let symbols_needing_data = market_data::get_symbols_needing_data(&conn)?.len();

    let template = MarketDataStatusTemplate {
        coverage,
        total_data_points,
        symbols_needing_data,
        refresh_message: None,
    };

    Ok(Html(template.render().unwrap()))
}

// Symbol detail page

#[derive(Template)]
#[template(path = "pages/market_data_symbol.html")]
pub struct MarketDataSymbolTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub symbol: String,
    pub coverage: Option<SymbolDataCoverage>,
    pub latest_price: Option<MarketData>,
    pub data_points: Vec<MarketData>,
    pub missing_ranges: Vec<(String, String)>,
}

pub async fn symbol_detail(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

    // Get coverage info for this symbol
    let all_coverage = market_data::get_symbol_coverage(&conn)?;
    let coverage = all_coverage.into_iter().find(|c| c.symbol == symbol);

    // Get all price data for this symbol
    let data_points = market_data::get_prices_for_symbol(&conn, &symbol)?;

    // Get latest price
    let latest_price = market_data::get_latest_price(&conn, &symbol)?;

    // Calculate missing date ranges
    let missing_ranges = calculate_missing_ranges(&data_points, coverage.as_ref());

    let template = MarketDataSymbolTemplate {
        title: format!("{} - Market Data", symbol),
        settings: app_settings,
        manifest: state.manifest.clone(),
        version: VERSION,
        symbol: symbol.clone(),
        coverage,
        latest_price,
        data_points,
        missing_ranges,
    };

    Ok(Html(template.render().unwrap()))
}

/// Calculate date ranges where data is missing
fn calculate_missing_ranges(
    data_points: &[MarketData],
    coverage: Option<&SymbolDataCoverage>,
) -> Vec<(String, String)> {
    let mut missing = Vec::new();

    let Some(cov) = coverage else {
        return missing;
    };

    if data_points.is_empty() {
        // All data is missing
        missing.push((
            cov.first_activity_date.clone(),
            chrono::Local::now().format("%Y-%m-%d").to_string(),
        ));
        return missing;
    }

    // Create a set of dates we have
    let mut dates_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for dp in data_points {
        dates_set.insert(dp.date.clone());
    }

    // Walk through the expected date range and find gaps
    let start = chrono::NaiveDate::parse_from_str(&cov.first_activity_date, "%Y-%m-%d");
    let end = chrono::NaiveDate::parse_from_str(
        &chrono::Local::now().format("%Y-%m-%d").to_string(),
        "%Y-%m-%d",
    );

    if let (Ok(start_date), Ok(end_date)) = (start, end) {
        let mut current = start_date;
        let mut gap_start: Option<chrono::NaiveDate> = None;

        while current <= end_date {
            let date_str = current.format("%Y-%m-%d").to_string();
            let is_weekday = current.weekday().num_days_from_monday() < 5;

            if is_weekday {
                if dates_set.contains(&date_str) {
                    // We have data for this day
                    if let Some(gs) = gap_start {
                        // End of a gap
                        let prev_day = current - chrono::Duration::days(1);
                        missing.push((
                            gs.format("%Y-%m-%d").to_string(),
                            prev_day.format("%Y-%m-%d").to_string(),
                        ));
                        gap_start = None;
                    }
                } else if gap_start.is_none() {
                    // Start of a gap
                    gap_start = Some(current);
                }
            }

            current += chrono::Duration::days(1);
        }

        // If we ended in a gap
        if let Some(gs) = gap_start {
            missing.push((
                gs.format("%Y-%m-%d").to_string(),
                end_date.format("%Y-%m-%d").to_string(),
            ));
        }
    }

    missing
}

// API endpoint for chart data

#[derive(Serialize)]
pub struct PriceChartData {
    pub date: String,
    pub price_cents: i64,
}

#[derive(Serialize)]
pub struct PriceChartResponse {
    pub symbol: String,
    pub data: Vec<PriceChartData>,
    pub missing_ranges: Vec<(String, String)>,
}

pub async fn symbol_chart_data(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<PriceChartResponse>> {
    let conn = state.db.get()?;

    // Get all price data for this symbol (ordered by date descending, we need ascending)
    let mut data_points = market_data::get_prices_for_symbol(&conn, &symbol)?;
    data_points.reverse(); // Now oldest first

    // Get coverage for missing ranges
    let all_coverage = market_data::get_symbol_coverage(&conn)?;
    let coverage = all_coverage.into_iter().find(|c| c.symbol == symbol);
    let missing_ranges = calculate_missing_ranges(&data_points, coverage.as_ref());

    let data: Vec<PriceChartData> = data_points
        .into_iter()
        .map(|dp| PriceChartData {
            date: dp.date,
            price_cents: dp.close_price_cents,
        })
        .collect();

    Ok(Json(PriceChartResponse {
        symbol,
        data,
        missing_ranges,
    }))
}

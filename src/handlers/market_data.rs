use askama::Template;
use axum::extract::State;
use axum::response::{Html, Redirect};

use crate::db::queries::{market_data, settings};
use crate::error::AppResult;
use crate::models::{Settings, SymbolDataCoverage};
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

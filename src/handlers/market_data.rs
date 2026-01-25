use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, Redirect};
use axum::Json;
use serde::{Deserialize, Serialize};

use chrono::Datelike;

use crate::db::queries::{api_logs, market_data, settings};
use crate::error::AppResult;
use crate::models::{MarketData, NewApiLog, Settings, SymbolDataCoverage};
use crate::services::market_data as market_data_service;
use crate::sort_utils::{SortDirection, Sortable, SortableColumn, TableSort};
use crate::state::{AppState, JsManifest, MarketDataRefreshState};
use crate::VERSION;

/// Sortable columns for the market data coverage table.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum MarketDataSortColumn {
    #[default]
    Symbol,
    ActivityRange,
    DataRange,
    DataPoints,
    Status,
}

impl SortableColumn for MarketDataSortColumn {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "symbol" => Some(Self::Symbol),
            "activityrange" => Some(Self::ActivityRange),
            "datarange" => Some(Self::DataRange),
            "datapoints" => Some(Self::DataPoints),
            "status" => Some(Self::Status),
            _ => None,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Symbol => "symbol",
            Self::ActivityRange => "activityrange",
            Self::DataRange => "datarange",
            Self::DataPoints => "datapoints",
            Self::Status => "status",
        }
    }

    fn sql_expression(&self) -> &'static str {
        // Not used for in-memory sorting
        ""
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct MarketDataFilterParams {
    pub sort: Option<String>,
    pub dir: Option<String>,
}

impl Sortable for MarketDataFilterParams {
    fn sort_by(&self) -> Option<&String> {
        self.sort.as_ref()
    }

    fn sort_dir(&self) -> Option<&String> {
        self.dir.as_ref()
    }
}

/// Sort market data coverage in-memory based on sort configuration.
fn sort_coverage(coverage: &mut [SymbolDataCoverage], sort: &TableSort<MarketDataSortColumn>) {
    coverage.sort_by(|a, b| {
        let cmp = match sort.column {
            MarketDataSortColumn::Symbol => a.symbol.cmp(&b.symbol),
            MarketDataSortColumn::ActivityRange => {
                a.first_activity_date.cmp(&b.first_activity_date)
            }
            MarketDataSortColumn::DataRange => a.first_data_date.cmp(&b.first_data_date),
            MarketDataSortColumn::DataPoints => a.data_points.cmp(&b.data_points),
            MarketDataSortColumn::Status => {
                // Sort by coverage status: Complete < Stale < No data
                let status_a = a.coverage_status();
                let status_b = b.coverage_status();
                status_a.cmp(&status_b)
            }
        };

        match sort.direction {
            SortDirection::Asc => cmp,
            SortDirection::Desc => cmp.reverse(),
        }
    });
}

#[derive(Template)]
#[template(path = "pages/market_data.html")]
pub struct MarketDataTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub coverage: Vec<SymbolDataCoverage>,
    pub total_data_points: i64,
    pub symbols_needing_data: usize,
    pub is_refreshing: bool,
    pub refresh_message: Option<String>,
    pub latest_log_id: i64,
    pub sort: TableSort<MarketDataSortColumn>,
}

pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<MarketDataFilterParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;
    let sort: TableSort<MarketDataSortColumn> = params.resolve_sort();

    let mut coverage = market_data::get_symbol_coverage(&conn)?;
    sort_coverage(&mut coverage, &sort);

    let total_data_points = market_data::count_market_data(&conn)?;
    let symbols_needing_data = market_data::get_symbols_needing_data(&conn)?.len();
    let latest_log_id = api_logs::get_latest_log_id(&conn).unwrap_or(0);

    // Get refresh state
    let (is_refreshing, refresh_message) = {
        let refresh_state = state.market_data_refresh.lock().unwrap();
        if refresh_state.is_refreshing {
            let msg = if let Some(ref symbol) = refresh_state.current_symbol {
                format!(
                    "Fetching {} ({}/{})...",
                    symbol,
                    refresh_state.processed_symbols + 1,
                    refresh_state.total_symbols
                )
            } else {
                format!(
                    "Fetching data ({}/{})...",
                    refresh_state.processed_symbols, refresh_state.total_symbols
                )
            };
            (true, Some(msg))
        } else {
            (false, None)
        }
    };

    let template = MarketDataTemplate {
        title: "Market Data".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        coverage,
        total_data_points,
        symbols_needing_data,
        is_refreshing,
        refresh_message,
        latest_log_id,
        sort,
    };

    Ok(Html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "partials/market_data_status.html")]
pub struct MarketDataStatusTemplate {
    pub icons: crate::filters::Icons,
    pub coverage: Vec<SymbolDataCoverage>,
    pub total_data_points: i64,
    pub symbols_needing_data: usize,
    pub is_refreshing: bool,
    pub refresh_message: Option<String>,
    pub progress_percent: u8,
}

pub async fn refresh(State(state): State<AppState>) -> AppResult<Redirect> {
    // Check if refresh is already in progress
    {
        let refresh_state = state.market_data_refresh.lock().unwrap();
        if refresh_state.is_refreshing {
            return Ok(Redirect::to("/trading/market-data"));
        }
    }

    let conn = state.db.get()?;

    // Get symbols that need data
    let symbols_to_fetch = market_data::get_symbols_needing_data(&conn)?;

    if symbols_to_fetch.is_empty() {
        return Ok(Redirect::to("/trading/market-data"));
    }

    // Set initial refresh state
    {
        let mut refresh_state = state.market_data_refresh.lock().unwrap();
        *refresh_state = MarketDataRefreshState {
            is_refreshing: true,
            processed_symbols: 0,
            total_symbols: symbols_to_fetch.len(),
            current_symbol: symbols_to_fetch.first().map(|(s, _, _)| s.clone()),
        };
    }

    // Spawn background task for fetching
    let state_clone = state.clone();
    tokio::spawn(async move {
        for (i, (symbol, start_date, end_date)) in symbols_to_fetch.iter().enumerate() {
            // Update current symbol in state
            {
                let mut refresh_state = state_clone.market_data_refresh.lock().unwrap();
                refresh_state.current_symbol = Some(symbol.clone());
            }

            let start_time = std::time::Instant::now();
            let request_params = serde_json::json!({
                "symbol": &symbol,
                "start_date": &start_date,
                "end_date": &end_date
            })
            .to_string();

            match market_data_service::fetch_historical_quotes(symbol, start_date, end_date).await {
                Ok(data) => {
                    let duration_ms = start_time.elapsed().as_millis() as i64;
                    if let Ok(conn) = state_clone.db.get() {
                        // Log success
                        let _ = api_logs::insert_api_log(
                            &conn,
                            &NewApiLog {
                                api_name: "yahoo_finance".to_string(),
                                action: "fetch_historical_quotes".to_string(),
                                symbol: Some(symbol.clone()),
                                request_params: request_params.clone(),
                                status: "success".to_string(),
                                response_summary: Some(format!(
                                    "Retrieved {} data points",
                                    data.len()
                                )),
                                response_details: Some(
                                    serde_json::json!({
                                        "data_points": data.len(),
                                        "first_date": data.first().map(|d| &d.date),
                                        "last_date": data.last().map(|d| &d.date),
                                    })
                                    .to_string(),
                                ),
                                duration_ms: Some(duration_ms),
                            },
                        );

                        if let Err(e) = market_data::insert_market_data_batch(&conn, &data) {
                            tracing::error!("Failed to insert market data for {}: {}", symbol, e);
                        } else {
                            tracing::info!("Fetched {} data points for {}", data.len(), symbol);
                        }

                        // Also fetch and store symbol metadata if not already cached
                        if market_data::get_symbol_metadata(&conn, symbol)
                            .ok()
                            .flatten()
                            .is_none()
                        {
                            if let Ok(Some(meta)) =
                                market_data_service::fetch_symbol_metadata(symbol).await
                            {
                                let _ = market_data::upsert_symbol_metadata(
                                    &conn,
                                    symbol,
                                    meta.short_name.as_deref(),
                                    meta.long_name.as_deref(),
                                    Some(&meta.exchange),
                                    Some(&meta.quote_type),
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    let duration_ms = start_time.elapsed().as_millis() as i64;
                    if let Ok(conn) = state_clone.db.get() {
                        // Log error
                        let _ = api_logs::insert_api_log(
                            &conn,
                            &NewApiLog {
                                api_name: "yahoo_finance".to_string(),
                                action: "fetch_historical_quotes".to_string(),
                                symbol: Some(symbol.clone()),
                                request_params,
                                status: "error".to_string(),
                                response_summary: Some(format!("{}", e)),
                                response_details: Some(format!("{:?}", e)),
                                duration_ms: Some(duration_ms),
                            },
                        );
                    }
                    tracing::error!("Failed to fetch market data for {}: {}", symbol, e);
                }
            }

            // Update progress after each symbol
            {
                let mut refresh_state = state_clone.market_data_refresh.lock().unwrap();
                refresh_state.processed_symbols = i + 1;
            }

            // Rate limiting between symbols
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        // Clear refresh state when done
        {
            let mut refresh_state = state_clone.market_data_refresh.lock().unwrap();
            *refresh_state = MarketDataRefreshState::default();
        }
    });

    Ok(Redirect::to("/trading/market-data"))
}

pub async fn refresh_symbol(
    State(state): State<AppState>,
    axum::extract::Path(symbol): axum::extract::Path<String>,
) -> AppResult<Redirect> {
    // Check if refresh is already in progress
    {
        let refresh_state = state.market_data_refresh.lock().unwrap();
        if refresh_state.is_refreshing {
            return Ok(Redirect::to("/trading/market-data"));
        }
    }

    let conn = state.db.get()?;

    // Get the date range for this symbol
    let symbols_needing = market_data::get_symbols_needing_data(&conn)?;
    let symbol_info = symbols_needing.iter().find(|(s, _, _)| s == &symbol);

    if let Some((_, start_date, end_date)) = symbol_info {
        let start = start_date.clone();
        let end = end_date.clone();
        let sym = symbol.clone();

        // Set refresh state for single symbol
        {
            let mut refresh_state = state.market_data_refresh.lock().unwrap();
            *refresh_state = MarketDataRefreshState {
                is_refreshing: true,
                processed_symbols: 0,
                total_symbols: 1,
                current_symbol: Some(sym.clone()),
            };
        }

        // Spawn background task
        let state_clone = state.clone();
        tokio::spawn(async move {
            let start_time = std::time::Instant::now();
            let request_params = serde_json::json!({
                "symbol": &sym,
                "start_date": &start,
                "end_date": &end
            })
            .to_string();

            match market_data_service::fetch_historical_quotes(&sym, &start, &end).await {
                Ok(data) => {
                    let duration_ms = start_time.elapsed().as_millis() as i64;
                    if let Ok(conn) = state_clone.db.get() {
                        // Log success
                        let _ = api_logs::insert_api_log(
                            &conn,
                            &NewApiLog {
                                api_name: "yahoo_finance".to_string(),
                                action: "fetch_historical_quotes".to_string(),
                                symbol: Some(sym.clone()),
                                request_params: request_params.clone(),
                                status: "success".to_string(),
                                response_summary: Some(format!(
                                    "Retrieved {} data points",
                                    data.len()
                                )),
                                response_details: Some(
                                    serde_json::json!({
                                        "data_points": data.len(),
                                        "first_date": data.first().map(|d| &d.date),
                                        "last_date": data.last().map(|d| &d.date),
                                    })
                                    .to_string(),
                                ),
                                duration_ms: Some(duration_ms),
                            },
                        );

                        if let Err(e) = market_data::insert_market_data_batch(&conn, &data) {
                            tracing::error!("Failed to insert market data for {}: {}", sym, e);
                        } else {
                            tracing::info!("Fetched {} data points for {}", data.len(), sym);
                        }

                        // Also fetch and store symbol metadata if not already cached
                        if market_data::get_symbol_metadata(&conn, &sym)
                            .ok()
                            .flatten()
                            .is_none()
                        {
                            if let Ok(Some(meta)) =
                                market_data_service::fetch_symbol_metadata(&sym).await
                            {
                                let _ = market_data::upsert_symbol_metadata(
                                    &conn,
                                    &sym,
                                    meta.short_name.as_deref(),
                                    meta.long_name.as_deref(),
                                    Some(&meta.exchange),
                                    Some(&meta.quote_type),
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    let duration_ms = start_time.elapsed().as_millis() as i64;
                    if let Ok(conn) = state_clone.db.get() {
                        // Log error
                        let _ = api_logs::insert_api_log(
                            &conn,
                            &NewApiLog {
                                api_name: "yahoo_finance".to_string(),
                                action: "fetch_historical_quotes".to_string(),
                                symbol: Some(sym.clone()),
                                request_params,
                                status: "error".to_string(),
                                response_summary: Some(format!("{}", e)),
                                response_details: Some(format!("{:?}", e)),
                                duration_ms: Some(duration_ms),
                            },
                        );
                    }
                    tracing::error!("Failed to fetch market data for {}: {}", sym, e);
                }
            }

            // Clear refresh state when done
            {
                let mut refresh_state = state_clone.market_data_refresh.lock().unwrap();
                *refresh_state = MarketDataRefreshState::default();
            }
        });
    }

    Ok(Redirect::to("/trading/market-data"))
}

pub async fn status(
    State(state): State<AppState>,
) -> AppResult<axum::response::Response<axum::body::Body>> {
    use axum::response::IntoResponse;

    let conn = state.db.get()?;

    let coverage = market_data::get_symbol_coverage(&conn)?;
    let total_data_points = market_data::count_market_data(&conn)?;
    let symbols_needing_data = market_data::get_symbols_needing_data(&conn)?.len();

    // Get refresh state
    let (is_refreshing, refresh_message, progress_percent) = {
        let refresh_state = state.market_data_refresh.lock().unwrap();
        if refresh_state.is_refreshing {
            let msg = if let Some(ref symbol) = refresh_state.current_symbol {
                format!(
                    "Fetching {} ({}/{})...",
                    symbol,
                    refresh_state.processed_symbols + 1,
                    refresh_state.total_symbols
                )
            } else {
                format!(
                    "Fetching data ({}/{})...",
                    refresh_state.processed_symbols, refresh_state.total_symbols
                )
            };
            (true, Some(msg), refresh_state.progress_percent())
        } else {
            (false, None, 0)
        }
    };

    let template = MarketDataStatusTemplate {
        icons: crate::filters::Icons,
        coverage,
        total_data_points,
        symbols_needing_data,
        is_refreshing,
        refresh_message,
        progress_percent,
    };

    let html = template.render().unwrap();

    // If refresh just completed, tell HTMX to refresh the page
    if !is_refreshing {
        Ok(([("hx-refresh", "true")], Html(html)).into_response())
    } else {
        Ok(Html(html).into_response())
    }
}

// Symbol detail page

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

    pub fn display_name_str(&self) -> &str {
        self.display_name().map(|s| s.as_str()).unwrap_or("")
    }
}

#[derive(Template)]
#[template(path = "pages/market_data_symbol.html")]
pub struct MarketDataSymbolTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub symbol: String,
    pub symbol_info: SymbolInfo,
    pub coverage: Option<SymbolDataCoverage>,
    pub latest_price: Option<MarketData>,
    pub data_points: Vec<MarketData>,
    pub data_points_total: usize,
    pub missing_ranges: Vec<(String, String)>,
}

pub async fn symbol_detail(
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

    // Get coverage info for this symbol
    let all_coverage = market_data::get_symbol_coverage(&conn)?;
    let coverage = all_coverage.into_iter().find(|c| c.symbol == symbol);

    // Get all price data for this symbol
    let all_data = market_data::get_prices_for_symbol(&conn, &symbol)?;

    // Get latest price
    let latest_price = market_data::get_latest_price(&conn, &symbol)?;

    // Calculate missing date ranges using ALL data (before limiting for display)
    let missing_ranges = calculate_missing_ranges(&all_data, coverage.as_ref());

    // Track total count before limiting for display
    let data_points_total = all_data.len();

    // Limit data points for display (most recent 100)
    let data_points: Vec<MarketData> = all_data.into_iter().take(100).collect();

    let display_name = symbol_info
        .display_name()
        .map(|n| format!("{} ({})", n, symbol))
        .unwrap_or_else(|| symbol.clone());

    let template = MarketDataSymbolTemplate {
        title: format!("{} - Market Data", display_name),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        symbol: symbol.clone(),
        symbol_info,
        coverage,
        latest_price,
        data_points,
        data_points_total,
        missing_ranges,
    };

    Ok(Html(template.render().unwrap()))
}

/// Minimum number of consecutive missing weekdays to count as a significant gap
/// Smaller gaps are likely market holidays (Christmas, New Year, etc.)
const MIN_GAP_WEEKDAYS: i64 = 5;

/// Calculate date ranges where data is missing (filtering out small gaps like holidays)
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
        let mut gap_weekday_count = 0i64;

        while current <= end_date {
            let date_str = current.format("%Y-%m-%d").to_string();
            let is_weekday = current.weekday().num_days_from_monday() < 5;

            if is_weekday {
                if dates_set.contains(&date_str) {
                    // We have data for this day - end any current gap
                    if let Some(gs) = gap_start {
                        // Only add gap if it's significant (>= MIN_GAP_WEEKDAYS)
                        if gap_weekday_count >= MIN_GAP_WEEKDAYS {
                            let prev_day = current - chrono::Duration::days(1);
                            missing.push((
                                gs.format("%Y-%m-%d").to_string(),
                                prev_day.format("%Y-%m-%d").to_string(),
                            ));
                        }
                        gap_start = None;
                        gap_weekday_count = 0;
                    }
                } else {
                    // Missing data for this weekday
                    if gap_start.is_none() {
                        gap_start = Some(current);
                        gap_weekday_count = 1;
                    } else {
                        gap_weekday_count += 1;
                    }
                }
            }

            current += chrono::Duration::days(1);
        }

        // If we ended in a significant gap
        if let Some(gs) = gap_start {
            if gap_weekday_count >= MIN_GAP_WEEKDAYS {
                missing.push((
                    gs.format("%Y-%m-%d").to_string(),
                    end_date.format("%Y-%m-%d").to_string(),
                ));
            }
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

pub async fn delete_symbol(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;
    market_data::delete_market_data_for_symbol(&conn, &symbol)?;
    Ok(Redirect::to("/trading/market-data"))
}

pub async fn delete_all(State(state): State<AppState>) -> AppResult<Redirect> {
    let conn = state.db.get()?;
    market_data::delete_all_market_data(&conn)?;
    Ok(Redirect::to("/trading/market-data"))
}

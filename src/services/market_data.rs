use crate::error::{AppError, AppResult};
use crate::models::NewMarketData;
use std::time::Duration;
use time::{Date, Month, OffsetDateTime, Time};
use tokio::time::sleep;
use tracing::{debug, info, warn};
use yahoo_finance_api as yahoo;

/// Delay between API requests to avoid rate limiting
const API_DELAY_MS: u64 = 500;

/// Fetch historical quotes for a symbol within a date range
/// Returns closing prices for each trading day
pub async fn fetch_historical_quotes(
    symbol: &str,
    start_date: &str,
    end_date: &str,
) -> AppResult<Vec<NewMarketData>> {
    debug!(symbol = %symbol, start_date = %start_date, end_date = %end_date, "Fetching historical quotes");

    let provider = yahoo::YahooConnector::new()
        .map_err(|e| AppError::Internal(format!("Failed to create Yahoo connector: {}", e)))?;

    // Parse dates from YYYY-MM-DD format
    let start = parse_date(start_date)?;
    let end = parse_date(end_date)?;

    // Convert to OffsetDateTime (UTC midnight)
    let start_utc = OffsetDateTime::new_utc(start, Time::MIDNIGHT);
    let end_utc = OffsetDateTime::new_utc(end, Time::from_hms(23, 59, 59).unwrap());

    // Fetch quotes
    let response = provider
        .get_quote_history(symbol, start_utc, end_utc)
        .await
        .map_err(|e| AppError::Internal(format!("Yahoo Finance API error: {}", e)))?;

    // Get currency from metadata (defaults to USD if not available)
    let currency = response
        .metadata()
        .ok()
        .and_then(|m| m.currency)
        .unwrap_or_else(|| "USD".to_string());

    let quotes = response
        .quotes()
        .map_err(|e| AppError::Internal(format!("Failed to parse quotes: {}", e)))?;

    // Convert to our market data format
    let market_data: Vec<NewMarketData> = quotes
        .iter()
        .filter_map(|quote| {
            let timestamp = quote.timestamp as i64;
            let datetime = OffsetDateTime::from_unix_timestamp(timestamp).ok()?;
            let date = format!(
                "{:04}-{:02}-{:02}",
                datetime.year(),
                datetime.month() as u8,
                datetime.day()
            );

            // Convert to cents
            let close_price_cents = (quote.close * 100.0).round() as i64;

            Some(NewMarketData {
                symbol: symbol.to_string(),
                date,
                close_price_cents,
                currency: currency.clone(),
            })
        })
        .collect();

    info!(symbol = %symbol, quote_count = market_data.len(), "Fetched historical quotes");
    Ok(market_data)
}

/// Fetch the latest quote for a symbol
pub async fn fetch_latest_quote(symbol: &str) -> AppResult<Option<NewMarketData>> {
    debug!(symbol = %symbol, "Fetching latest quote");

    let provider = yahoo::YahooConnector::new()
        .map_err(|e| AppError::Internal(format!("Failed to create Yahoo connector: {}", e)))?;

    let response = provider
        .get_latest_quotes(symbol, "1d")
        .await
        .map_err(|e| {
            warn!(symbol = %symbol, error = %e, "Failed to fetch latest quote");
            AppError::Internal(format!("Yahoo Finance API error: {}", e))
        })?;

    // Get currency from metadata (defaults to USD if not available)
    let currency = response
        .metadata()
        .ok()
        .and_then(|m| m.currency)
        .unwrap_or_else(|| "USD".to_string());

    let quote = match response.last_quote() {
        Ok(q) => q,
        Err(_) => return Ok(None),
    };

    let timestamp = quote.timestamp as i64;
    let datetime = OffsetDateTime::from_unix_timestamp(timestamp)
        .map_err(|_| AppError::Internal("Invalid timestamp".into()))?;
    let date = format!(
        "{:04}-{:02}-{:02}",
        datetime.year(),
        datetime.month() as u8,
        datetime.day()
    );
    let close_price_cents = (quote.close * 100.0).round() as i64;

    Ok(Some(NewMarketData {
        symbol: symbol.to_string(),
        date,
        close_price_cents,
        currency,
    }))
}

/// Symbol metadata from Yahoo Finance
#[derive(Debug, Clone)]
pub struct SymbolMetadata {
    pub symbol: String,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub exchange: String,
    pub quote_type: String,
}

/// Fetch metadata for a symbol (name, exchange, type)
pub async fn fetch_symbol_metadata(symbol: &str) -> AppResult<Option<SymbolMetadata>> {
    debug!(symbol = %symbol, "Fetching symbol metadata");

    let provider = yahoo::YahooConnector::new()
        .map_err(|e| AppError::Internal(format!("Failed to create Yahoo connector: {}", e)))?;

    let response = provider
        .search_ticker_opt(symbol)
        .await
        .map_err(|e| AppError::Internal(format!("Yahoo Finance API error: {}", e)))?;

    // Find exact match for the symbol
    let quote = response
        .quotes
        .into_iter()
        .find(|q| q.symbol.to_uppercase() == symbol.to_uppercase());

    if quote.is_some() {
        debug!(symbol = %symbol, "Found symbol metadata");
    } else {
        debug!(symbol = %symbol, "Symbol metadata not found");
    }

    Ok(quote.map(|q| SymbolMetadata {
        symbol: q.symbol,
        short_name: q.short_name,
        long_name: q.long_name,
        exchange: q.exchange,
        quote_type: q.quote_type,
    }))
}

/// Parse a date string in YYYY-MM-DD format
fn parse_date(date_str: &str) -> AppResult<Date> {
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() != 3 {
        return Err(AppError::Validation(format!(
            "Invalid date format: {}",
            date_str
        )));
    }

    let year: i32 = parts[0]
        .parse()
        .map_err(|_| AppError::Validation(format!("Invalid year: {}", parts[0])))?;
    let month: u8 = parts[1]
        .parse()
        .map_err(|_| AppError::Validation(format!("Invalid month: {}", parts[1])))?;
    let day: u8 = parts[2]
        .parse()
        .map_err(|_| AppError::Validation(format!("Invalid day: {}", parts[2])))?;

    let month = Month::try_from(month)
        .map_err(|_| AppError::Validation(format!("Invalid month: {}", month)))?;

    Date::from_calendar_date(year, month, day)
        .map_err(|e| AppError::Validation(format!("Invalid date: {}", e)))
}

/// Fetch quotes for multiple symbols with rate limiting
pub async fn fetch_quotes_for_symbols(
    symbols: &[(&str, &str, &str)], // (symbol, start_date, end_date)
) -> Vec<(String, AppResult<Vec<NewMarketData>>)> {
    info!(
        symbol_count = symbols.len(),
        "Fetching quotes for multiple symbols"
    );
    let mut results = Vec::new();

    for (i, (symbol, start_date, end_date)) in symbols.iter().enumerate() {
        // Add delay between requests (except for the first one)
        if i > 0 {
            sleep(Duration::from_millis(API_DELAY_MS)).await;
        }

        let result = fetch_historical_quotes(symbol, start_date, end_date).await;
        results.push((symbol.to_string(), result));
    }

    results
}

/// Fetch latest quotes for multiple symbols with rate limiting
pub async fn fetch_latest_quotes_for_symbols(
    symbols: &[&str],
) -> Vec<(String, AppResult<Option<NewMarketData>>)> {
    info!(
        symbol_count = symbols.len(),
        "Fetching latest quotes for multiple symbols"
    );
    let mut results = Vec::new();

    for (i, symbol) in symbols.iter().enumerate() {
        // Add delay between requests (except for the first one)
        if i > 0 {
            sleep(Duration::from_millis(API_DELAY_MS)).await;
        }

        let result = fetch_latest_quote(symbol).await;
        results.push((symbol.to_string(), result));
    }

    results
}

use serde::{Deserialize, Serialize};

/// A single market data point (closing price for a symbol on a date)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub id: i64,
    pub symbol: String,
    pub date: String,
    pub close_price_cents: i64,
    pub currency: String,
    pub fetched_at: String,
}

impl MarketData {
    pub fn close_price_display(&self) -> String {
        let dollars = self.close_price_cents / 100;
        let cents = self.close_price_cents % 100;
        format!("{}.{:02}", dollars, cents)
    }

    pub fn close_price_formatted(&self) -> String {
        format!(
            "{}{}",
            currency_symbol(&self.currency),
            self.close_price_display()
        )
    }
}

/// New market data for insertion
#[derive(Debug, Clone)]
pub struct NewMarketData {
    pub symbol: String,
    pub date: String,
    pub close_price_cents: i64,
    pub currency: String,
}

/// Summary of market data coverage for a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolDataCoverage {
    pub symbol: String,
    pub currency: String,
    pub first_activity_date: String,
    pub last_activity_date: String,
    pub first_data_date: Option<String>,
    pub last_data_date: Option<String>,
    pub data_points: i64,
    pub missing_days: i64,
    pub has_current_price: bool,
}

impl SymbolDataCoverage {
    pub fn coverage_status(&self) -> &'static str {
        if self.data_points == 0 {
            "No data"
        } else if self.has_current_price {
            // If we have recent price data (within 5 days), consider it complete
            // Gaps up to 5 days are expected due to weekends and holidays
            "Complete"
        } else {
            "Stale"
        }
    }

    pub fn status_color(&self) -> &'static str {
        match self.coverage_status() {
            "Complete" => "text-green-600 dark:text-green-400",
            "Stale" => "text-orange-600 dark:text-orange-400",
            _ => "text-red-600 dark:text-red-400",
        }
    }
}

fn currency_symbol(currency: &str) -> &'static str {
    match currency.to_uppercase().as_str() {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => "¥",
        "CNY" => "¥",
        "CAD" => "C$",
        "AUD" => "A$",
        "CHF" => "CHF ",
        "INR" => "₹",
        "BRL" => "R$",
        "MXN" => "MX$",
        _ => "$",
    }
}

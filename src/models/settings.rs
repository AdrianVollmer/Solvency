use crate::filters;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub theme: String,
    pub currency: String,
    pub date_format: String,
    pub page_size: i64,
    pub locale: String,
}

impl Settings {
    pub fn from_map(map: HashMap<String, String>) -> Self {
        Self {
            theme: map.get("theme").cloned().unwrap_or_else(|| "system".into()),
            currency: map.get("currency").cloned().unwrap_or_else(|| "USD".into()),
            date_format: map
                .get("date_format")
                .cloned()
                .unwrap_or_else(|| "YYYY-MM-DD".into()),
            page_size: map
                .get("page_size")
                .and_then(|s| s.parse().ok())
                .unwrap_or(25),
            locale: map.get("locale").cloned().unwrap_or_else(|| "en-US".into()),
        }
    }

    pub fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("theme".into(), self.theme.clone());
        map.insert("currency".into(), self.currency.clone());
        map.insert("date_format".into(), self.date_format.clone());
        map.insert("page_size".into(), self.page_size.to_string());
        map.insert("locale".into(), self.locale.clone());
        map
    }

    pub fn is_theme(&self, value: &str) -> bool {
        self.theme == value
    }

    pub fn is_currency(&self, value: &str) -> bool {
        self.currency == value
    }

    pub fn is_date_format(&self, value: &str) -> bool {
        self.date_format == value
    }

    pub fn is_locale(&self, value: &str) -> bool {
        self.locale == value
    }

    pub fn is_dark(&self) -> bool {
        self.theme == "dark"
    }

    /// Format a monetary amount (in cents) with proper locale formatting and color coding.
    /// Returns HTML with Tailwind color classes:
    /// - Positive: green
    /// - Negative: red
    /// - Zero: default text color
    pub fn format_money(&self, cents: &i64) -> String {
        filters::format_money(*cents, &self.currency, &self.locale)
    }

    /// Format a monetary amount (in cents) as plain text without HTML/colors.
    pub fn format_money_plain(&self, cents: &i64) -> String {
        filters::format_money_plain(*cents, &self.currency, &self.locale)
    }

    /// Format a monetary amount (in cents) with a specific currency and color coding.
    /// Useful for trading items that have their own currency field.
    pub fn format_money_with_currency(&self, cents: &i64, currency: &str) -> String {
        filters::format_money(*cents, currency, &self.locale)
    }

    /// Format a monetary amount (in cents) as neutral text (no sign, no color).
    /// Useful for prices, fees, and other amounts that shouldn't show +/-.
    pub fn format_money_neutral(&self, cents: &i64) -> String {
        filters::format_money_neutral(*cents, &self.currency, &self.locale)
    }

    /// Format a monetary amount (in cents) as neutral text with a specific currency.
    /// Useful for trading prices/fees that have their own currency field.
    pub fn format_money_neutral_with_currency(&self, cents: &i64, currency: &str) -> String {
        filters::format_money_neutral(*cents, currency, &self.locale)
    }

    /// Format a percentage value with locale-aware decimal separator.
    /// Shows sign (+/-) and two decimal places.
    pub fn format_percent(&self, value: &f64) -> String {
        filters::format_percent(*value, &self.locale)
    }
}

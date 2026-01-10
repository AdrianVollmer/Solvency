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
}

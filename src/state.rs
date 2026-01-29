use crate::config::{AuthMode, Config};
use crate::db::queries::settings as db_settings;
use crate::db::DbPool;
use crate::error::AppResult;
use crate::models::Settings;
use crate::xsrf::XsrfToken;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// State for tracking market data refresh operations
#[derive(Clone, Debug, Default)]
pub struct MarketDataRefreshState {
    pub is_refreshing: bool,
    pub processed_symbols: usize,
    pub total_symbols: usize,
    pub current_symbol: Option<String>,
}

impl MarketDataRefreshState {
    pub fn progress_percent(&self) -> u8 {
        if self.total_symbols == 0 {
            return 0;
        }
        ((self.processed_symbols as f64 / self.total_symbols as f64) * 100.0) as u8
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub config: Arc<Config>,
    pub manifest: JsManifest,
    pub xsrf_token: XsrfToken,
    pub market_data_refresh: Arc<Mutex<MarketDataRefreshState>>,
}

impl AppState {
    /// Load settings from the database with runtime auth state populated.
    pub fn load_settings(&self) -> AppResult<Settings> {
        let conn = self.db.get()?;
        let mut settings = db_settings::get_settings(&conn)?;
        settings.is_authenticated = matches!(self.config.auth_mode, AuthMode::Password(_));
        Ok(settings)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct JsManifest(HashMap<String, String>);

impl JsManifest {
    pub fn load(static_path: &Path) -> Self {
        let path = static_path.join("js/dist/manifest.json");
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => {
                tracing::warn!(
                    "manifest.json not found at {}, using empty manifest",
                    path.display()
                );
                Self::default()
            }
        }
    }

    pub fn get(&self, name: &str) -> String {
        self.0
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }
}

use crate::config::Config;
use crate::db::DbPool;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub config: Arc<Config>,
    pub manifest: JsManifest,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct JsManifest(HashMap<String, String>);

impl JsManifest {
    pub fn load() -> Self {
        let path = "static/js/dist/manifest.json";
        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => {
                tracing::warn!("manifest.json not found at {}, using empty manifest", path);
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

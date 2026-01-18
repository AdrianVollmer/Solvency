use serde::{Deserialize, Serialize};

/// A single API log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiLog {
    pub id: i64,
    pub api_name: String,
    pub action: String,
    pub symbol: Option<String>,
    pub request_params: String,
    pub status: String,
    pub response_summary: Option<String>,
    pub response_details: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

impl ApiLog {
    pub fn is_error(&self) -> bool {
        self.status == "error"
    }

    pub fn status_color(&self) -> &'static str {
        if self.is_error() {
            "text-red-600 dark:text-red-400"
        } else {
            "text-green-600 dark:text-green-400"
        }
    }
}

/// New API log for insertion
#[derive(Debug, Clone)]
pub struct NewApiLog {
    pub api_name: String,
    pub action: String,
    pub symbol: Option<String>,
    pub request_params: String,
    pub status: String,
    pub response_summary: Option<String>,
    pub response_details: Option<String>,
    pub duration_ms: Option<i64>,
}

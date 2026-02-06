use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Status of an AI categorization session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiCategorizationStatus {
    Pending,
    Processing,
    Completed,
    Cancelled,
    Failed,
}

impl AiCategorizationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Processing => "Processing...",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
            Self::Failed => "Failed",
        }
    }
}

impl FromStr for AiCategorizationStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            _ => Err(()),
        }
    }
}

/// Status of an individual categorization result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiResultStatus {
    Pending,
    Applied,
    Rejected,
    Skipped,
    Error,
}

impl AiResultStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::Rejected => "rejected",
            Self::Skipped => "skipped",
            Self::Error => "error",
        }
    }
}

impl FromStr for AiResultStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "applied" => Ok(Self::Applied),
            "rejected" => Ok(Self::Rejected),
            "skipped" => Ok(Self::Skipped),
            "error" => Ok(Self::Error),
            _ => Err(()),
        }
    }
}

/// AI categorization session tracking background processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCategorizationSession {
    pub id: String,
    pub status: AiCategorizationStatus,
    pub provider: String,
    pub model: String,
    pub total_transactions: i64,
    pub processed_transactions: i64,
    pub categorized_count: i64,
    pub skipped_count: i64,
    pub error_count: i64,
    pub errors: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl AiCategorizationSession {
    pub fn progress_percent(&self) -> i64 {
        if self.total_transactions == 0 {
            0
        } else {
            (self.processed_transactions * 100) / self.total_transactions
        }
    }

    pub fn is_processing(&self) -> bool {
        matches!(
            self.status,
            AiCategorizationStatus::Pending | AiCategorizationStatus::Processing
        )
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.status, AiCategorizationStatus::Pending)
    }

    pub fn is_completed(&self) -> bool {
        matches!(self.status, AiCategorizationStatus::Completed)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.status, AiCategorizationStatus::Failed)
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self.status, AiCategorizationStatus::Cancelled)
    }
}

/// Individual categorization result for a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCategorizationResult {
    pub id: i64,
    pub session_id: String,
    pub transaction_id: i64,
    pub original_category_id: Option<i64>,
    pub suggested_category_id: Option<i64>,
    pub confidence: Option<f64>,
    pub ai_reasoning: Option<String>,
    pub status: AiResultStatus,
    pub error: Option<String>,
    pub created_at: String,
}

/// Result with joined transaction and category data for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCategorizationResultWithDetails {
    pub result: AiCategorizationResult,
    pub transaction_date: String,
    pub transaction_description: String,
    pub transaction_amount_cents: i64,
    pub transaction_currency: String,
    pub original_category_name: Option<String>,
    pub suggested_category_name: Option<String>,
    pub suggested_category_path: Option<String>,
}

impl AiCategorizationResultWithDetails {
    pub fn confidence_percent(&self) -> Option<i64> {
        self.result.confidence.map(|c| (c * 100.0).round() as i64)
    }

    pub fn confidence_color(&self) -> &'static str {
        match self.result.confidence {
            Some(c) if c >= 0.8 => "text-green-600 dark:text-green-400",
            Some(c) if c >= 0.5 => "text-yellow-600 dark:text-yellow-400",
            Some(_) => "text-red-600 dark:text-red-400",
            None => "text-gray-500 dark:text-gray-400",
        }
    }

    pub fn is_pending(&self) -> bool {
        self.result.status == AiResultStatus::Pending
    }

    pub fn is_applied(&self) -> bool {
        self.result.status == AiResultStatus::Applied
    }

    pub fn is_rejected(&self) -> bool {
        self.result.status == AiResultStatus::Rejected
    }
}

/// AI provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AiProvider {
    OpenAi,
    #[default]
    Ollama,
    Anthropic,
}

impl AiProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Ollama => "ollama",
            Self::Anthropic => "anthropic",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Ollama => "Ollama (Local)",
            Self::Anthropic => "Anthropic",
        }
    }

    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Ollama => "http://localhost:11434",
            Self::Anthropic => "https://api.anthropic.com",
        }
    }

    pub fn requires_api_key(&self) -> bool {
        match self {
            Self::OpenAi | Self::Anthropic => true,
            Self::Ollama => false,
        }
    }
}

impl FromStr for AiProvider {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "openai" => Ok(Self::OpenAi),
            "ollama" => Ok(Self::Ollama),
            "anthropic" => Ok(Self::Anthropic),
            _ => Err(()),
        }
    }
}

/// AI settings stored in the settings table
#[derive(Debug, Clone, Default)]
pub struct AiSettings {
    pub provider: AiProvider,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl AiSettings {
    pub fn from_settings(settings: &std::collections::HashMap<String, String>) -> Self {
        let provider: AiProvider = settings
            .get("ai_provider")
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();

        let base_url = settings
            .get("ai_base_url")
            .cloned()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| provider.default_base_url().to_string());

        let api_key = settings.get("ai_api_key").cloned().unwrap_or_default();

        let model = settings
            .get("ai_model")
            .cloned()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_model_for_provider(provider).to_string());

        Self {
            provider,
            base_url,
            api_key,
            model,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.base_url.is_empty()
            && !self.model.is_empty()
            && (self.provider == AiProvider::Ollama || !self.api_key.is_empty())
    }
}

fn default_model_for_provider(provider: AiProvider) -> &'static str {
    match provider {
        AiProvider::OpenAi => "gpt-4o-mini",
        AiProvider::Ollama => "llama3.2",
        AiProvider::Anthropic => "claude-sonnet-4-20250514",
    }
}

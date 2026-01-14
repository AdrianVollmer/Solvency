use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    Parsing,
    Preview,
    Importing,
    Completed,
    Failed,
}

impl ImportStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Parsing => "parsing",
            Self::Preview => "preview",
            Self::Importing => "importing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Parsing => "Parsing files...",
            Self::Preview => "Preview",
            Self::Importing => "Importing...",
            Self::Completed => "Import Complete",
            Self::Failed => "Import Failed",
        }
    }
}

impl FromStr for ImportStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "parsing" => Ok(Self::Parsing),
            "preview" => Ok(Self::Preview),
            "importing" => Ok(Self::Importing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSession {
    pub id: String,
    pub status: ImportStatus,
    pub total_rows: i64,
    pub processed_rows: i64,
    pub error_count: i64,
    pub errors: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ImportSession {
    pub fn progress_percent(&self) -> i64 {
        if self.total_rows == 0 {
            0
        } else {
            (self.processed_rows * 100) / self.total_rows
        }
    }

    pub fn is_processing(&self) -> bool {
        matches!(self.status, ImportStatus::Parsing | ImportStatus::Importing)
    }

    pub fn is_parsing(&self) -> bool {
        matches!(self.status, ImportStatus::Parsing)
    }

    pub fn is_preview(&self) -> bool {
        matches!(self.status, ImportStatus::Preview)
    }

    pub fn is_importing(&self) -> bool {
        matches!(self.status, ImportStatus::Importing)
    }

    pub fn is_completed(&self) -> bool {
        matches!(self.status, ImportStatus::Completed)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.status, ImportStatus::Failed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportRowStatus {
    Pending,
    Imported,
    Error,
}

impl ImportRowStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Imported => "imported",
            Self::Error => "error",
        }
    }
}

impl FromStr for ImportRowStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "imported" => Ok(Self::Imported),
            "error" => Ok(Self::Error),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRow {
    pub id: i64,
    pub session_id: String,
    pub row_index: i64,
    pub data: crate::services::csv_parser::ParsedExpense,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

use askama::Template;

use crate::models::Settings;
use crate::state::JsManifest;

/// Status of a single item in an import preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportPreviewStatus {
    Ok,
    Skipped,
}

/// One row in the import preview table.
#[derive(Debug, Clone)]
pub struct ImportPreviewItem {
    pub status: ImportPreviewStatus,
    /// Human-readable reason the item will be skipped (empty for Ok items).
    pub reason: String,
    /// Column values for display, matching the order of `columns`.
    pub cells: Vec<String>,
}

impl ImportPreviewItem {
    pub fn is_skipped(&self) -> bool {
        self.status == ImportPreviewStatus::Skipped
    }
}

/// Form payload sent by the client-side hidden form.
#[derive(Debug, serde::Deserialize)]
pub struct ImportPreviewForm {
    pub data: String,
}

/// Generic import preview page shared by all JSON importers.
#[derive(Template)]
#[template(path = "pages/import_preview.html")]
pub struct ImportPreviewTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub resource_name: String,
    pub back_url: String,
    pub import_url: String,
    pub columns: Vec<String>,
    pub items: Vec<ImportPreviewItem>,
    pub ok_count: usize,
    pub skip_count: usize,
    pub raw_json: String,
}

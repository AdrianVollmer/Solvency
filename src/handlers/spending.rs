use askama::Template;
use axum::extract::{Query, State};
use axum::response::Html;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::date_utils::{DatePreset, DateRange};
use crate::db::queries::{categories, settings};
use crate::error::{AppResult, RenderHtml};
use crate::models::category::CategoryWithPath;
use crate::models::Settings;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Debug, Default, Deserialize)]
pub struct SpendingFilterParams {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub preset: Option<String>,
    pub tab: Option<String>,
}

impl SpendingFilterParams {
    pub fn resolve_date_range(&self) -> DateRange {
        if let Some(preset_str) = &self.preset {
            preset_str
                .parse::<DatePreset>()
                .map(DateRange::from_preset)
                .unwrap_or_default()
        } else if let (Some(from), Some(to)) = (&self.from_date, &self.to_date) {
            if let (Ok(from_date), Ok(to_date)) = (
                NaiveDate::parse_from_str(from, "%Y-%m-%d"),
                NaiveDate::parse_from_str(to, "%Y-%m-%d"),
            ) {
                DateRange::from_dates(from_date, to_date)
            } else {
                DateRange::default()
            }
        } else {
            DateRange::default()
        }
    }
}

#[derive(Template)]
#[template(path = "pages/spending.html")]
pub struct SpendingTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub date_range: DateRange,
    pub presets: &'static [DatePreset],
    pub active_tab: String,
    pub categories: Vec<CategoryWithPath>,
}

pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<SpendingFilterParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

    let date_range = params.resolve_date_range();

    let active_tab = match params.tab.as_deref() {
        Some("time") => "time".to_string(),
        Some("monthly") => "monthly".to_string(),
        _ => "category".to_string(),
    };

    let cats = categories::list_categories_with_path(&conn)?;

    let template = SpendingTemplate {
        title: "Spending".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        date_range,
        presets: DatePreset::all(),
        active_tab,
        categories: cats,
    };

    template.render_html()
}

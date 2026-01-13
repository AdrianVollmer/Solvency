use askama::Template;
use axum::extract::{Query, State};
use axum::response::Html;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::date_utils::{DatePreset, DateRange};
use crate::db::queries::settings;
use crate::error::AppResult;
use crate::models::Settings;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Debug, Default, Deserialize)]
pub struct AnalyticsFilterParams {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub preset: Option<String>,
}

impl AnalyticsFilterParams {
    pub fn resolve_date_range(&self) -> DateRange {
        if let Some(preset_str) = &self.preset {
            DatePreset::from_str(preset_str)
                .map(DateRange::from_preset)
                .unwrap_or_else(DateRange::default)
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
#[template(path = "pages/analytics.html")]
pub struct AnalyticsTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub date_range: DateRange,
    pub presets: &'static [DatePreset],
}

pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<AnalyticsFilterParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let settings_map = settings::get_all_settings(&conn)?;
    let app_settings = Settings::from_map(settings_map);

    let date_range = params.resolve_date_range();

    let template = AnalyticsTemplate {
        title: "Analytics".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        version: VERSION,
        date_range,
        presets: DatePreset::all(),
    };

    Ok(Html(template.render().unwrap()))
}

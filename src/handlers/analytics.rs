use askama::Template;
use axum::extract::State;
use axum::response::Html;

use crate::db::queries::settings;
use crate::error::AppResult;
use crate::models::Settings;
use crate::state::{AppState, JsManifest};

#[derive(Template)]
#[template(path = "pages/analytics.html")]
pub struct AnalyticsTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let settings_map = settings::get_all_settings(&conn)?;
    let app_settings = Settings::from_map(settings_map);

    let template = AnalyticsTemplate {
        title: "Analytics".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
    };

    Ok(Html(template.render().unwrap()))
}

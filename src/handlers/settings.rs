use askama::Template;
use axum::extract::State;
use axum::response::Html;
use axum::Form;
use serde::Deserialize;

use crate::db::queries::settings;
use crate::error::AppResult;
use crate::models::Settings;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/settings.html")]
pub struct SettingsTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
}

#[derive(Template)]
#[template(path = "partials/settings_saved.html")]
pub struct SettingsSavedTemplate {
    pub icons: crate::filters::Icons,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct SettingsFormData {
    pub theme: String,
    pub currency: String,
    pub date_format: String,
    pub page_size: String,
    pub locale: String,
}

#[derive(Debug, Deserialize)]
pub struct ThemeFormData {
    pub theme: String,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

    let template = SettingsTemplate {
        title: "Settings".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn update(
    State(state): State<AppState>,
    Form(form): Form<SettingsFormData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    settings::set_setting(&conn, "theme", &form.theme)?;
    settings::set_setting(&conn, "currency", &form.currency)?;
    settings::set_setting(&conn, "date_format", &form.date_format)?;
    settings::set_setting(&conn, "page_size", &form.page_size)?;
    settings::set_setting(&conn, "locale", &form.locale)?;

    let template = SettingsSavedTemplate {
        icons: crate::filters::Icons,
        message: "Settings saved successfully".into(),
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn toggle_theme(
    State(state): State<AppState>,
    Form(form): Form<ThemeFormData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    settings::set_setting(&conn, "theme", &form.theme)?;

    Ok(Html(String::new()))
}

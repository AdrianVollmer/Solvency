use askama::Template;
use axum::extract::State;
use axum::response::Html;

use crate::db::queries::{settings, trading};
use crate::error::AppResult;
use crate::models::{Position, Settings};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/trading_positions.html")]
pub struct TradingPositionsTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub positions: Vec<Position>,
    pub cash_positions: Vec<Position>,
    pub security_positions: Vec<Position>,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let settings_map = settings::get_all_settings(&conn)?;
    let app_settings = Settings::from_map(settings_map);

    let all_positions = trading::get_positions(&conn)?;

    // Separate cash and security positions
    let (cash_positions, security_positions): (Vec<_>, Vec<_>) =
        all_positions.iter().cloned().partition(|p| p.is_cash());

    let template = TradingPositionsTemplate {
        title: "Positions".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        version: VERSION,
        positions: all_positions,
        cash_positions,
        security_positions,
    };

    Ok(Html(template.render().unwrap()))
}

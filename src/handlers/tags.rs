use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, Json};
use axum::Form;
use serde::Deserialize;

use crate::db::queries::{settings, tags};
use crate::error::{AppError, AppResult};
use crate::models::{NewTag, Settings, Tag};
use crate::state::{AppState, JsManifest};

#[derive(Template)]
#[template(path = "pages/tags.html")]
pub struct TagsTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub tags: Vec<Tag>,
}

#[derive(Template)]
#[template(path = "components/tag_badge.html")]
pub struct TagBadgeTemplate {
    pub tag: Tag,
}

#[derive(Debug, Deserialize)]
pub struct TagFormData {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TagSearchParams {
    pub q: Option<String>,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let settings_map = settings::get_all_settings(&conn)?;
    let app_settings = Settings::from_map(settings_map);

    let tag_list = tags::list_tags(&conn)?;

    let template = TagsTemplate {
        title: "Tags".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        tags: tag_list,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<TagSearchParams>,
) -> AppResult<Json<Vec<Tag>>> {
    let conn = state.db.get()?;

    let query = params.q.unwrap_or_default();
    let tag_list = if query.is_empty() {
        tags::list_tags(&conn)?
    } else {
        tags::search_tags(&conn, &query)?
    };

    Ok(Json(tag_list))
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<TagFormData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let new_tag = NewTag {
        name: form.name,
        color: form.color.unwrap_or_else(|| "#6b7280".into()),
    };

    let id = tags::create_tag(&conn, &new_tag)?;

    let tag = tags::get_tag(&conn, id)?
        .ok_or_else(|| AppError::Internal("Failed to retrieve created tag".into()))?;

    let template = TagBadgeTemplate { tag };

    Ok(Html(template.render().unwrap()))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    tags::delete_tag(&conn, id)?;

    Ok(Html(String::new()))
}

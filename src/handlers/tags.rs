use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, Json, Redirect};
use axum::Form;
use serde::Deserialize;

use crate::db::queries::tags;
use crate::error::{AppError, AppResult, RenderHtml};
use crate::models::{NewTag, Settings, Tag, TagStyle, TAG_PALETTE};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/tag_form.html")]
pub struct TagFormTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub editing: Option<Tag>,
    pub palette: &'static [(&'static str, &'static str)],
}

#[derive(Template)]
#[template(path = "components/tag_badge.html")]
pub struct TagBadgeTemplate {
    pub icons: crate::filters::Icons,
    pub tag: Tag,
}

#[derive(Debug, Deserialize)]
pub struct TagFormData {
    pub name: String,
    pub color: Option<String>,
    pub style: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TagSearchParams {
    pub q: Option<String>,
}

pub async fn new_form(State(state): State<AppState>) -> AppResult<Html<String>> {
    let app_settings = state.load_settings()?;

    let template = TagFormTemplate {
        title: "Add Tag".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        editing: None,
        palette: TAG_PALETTE,
    };

    template.render_html()
}

pub async fn edit_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    let tag =
        tags::get_tag(&conn, id)?.ok_or_else(|| AppError::NotFound("Tag not found".into()))?;

    let template = TagFormTemplate {
        title: format!("Edit Tag: {}", tag.name),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        editing: Some(tag),
        palette: TAG_PALETTE,
    };

    template.render_html()
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<TagFormData>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    let updated_tag = NewTag {
        name: form.name,
        color: form.color.unwrap_or_else(|| "#6b7280".into()),
        style: form.style.map(|s| TagStyle::parse(&s)).unwrap_or_default(),
    };

    tags::update_tag(&conn, id, &updated_tag)?;

    Ok(Redirect::to("/manage?tab=tags"))
}

pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<TagSearchParams>,
) -> AppResult<Json<Vec<Tag>>> {
    let conn = state.db.get()?;

    let query = params.q.unwrap_or_default();
    let tag_list = if query.is_empty() {
        state.cached_tags()?
    } else {
        tags::search_tags(&conn, &query)?
    };

    Ok(Json(tag_list))
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<TagFormData>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    let new_tag = NewTag {
        name: form.name,
        color: form.color.unwrap_or_else(|| "#6b7280".into()),
        style: form.style.map(|s| TagStyle::parse(&s)).unwrap_or_default(),
    };

    tags::create_tag(&conn, &new_tag)?;

    Ok(Redirect::to("/manage?tab=tags"))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    tags::delete_tag(&conn, id)?;

    Ok(Html(String::new()))
}

pub async fn delete_all(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    tags::delete_all_tags(&conn)?;

    Ok(Html(String::new()))
}

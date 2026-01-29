use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Json, Redirect};
use axum::Form;
use serde::{Deserialize, Serialize};

use crate::db::queries::tags;
use crate::error::{AppError, AppResult, RenderHtml};
use crate::models::{NewTag, Settings, Tag, TagStyle};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/tags.html")]
pub struct TagsTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub tags: Vec<Tag>,
    pub delete_count: i64,
}

#[derive(Template)]
#[template(path = "pages/tag_form.html")]
pub struct TagFormTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
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

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = state.load_settings()?;

    let tag_list = tags::list_tags(&conn)?;

    let template = TagsTemplate {
        title: "Tags".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        delete_count: tag_list.len() as i64,
        tags: tag_list,
    };

    template.render_html()
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
    };

    template.render_html()
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
) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    let new_tag = NewTag {
        name: form.name,
        color: form.color.unwrap_or_else(|| "#6b7280".into()),
        style: form.style.map(|s| TagStyle::parse(&s)).unwrap_or_default(),
    };

    tags::create_tag(&conn, &new_tag)?;

    Ok(Redirect::to("/tags"))
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

#[derive(Serialize)]
struct TagExport {
    name: String,
    color: String,
    style: TagStyle,
}

pub async fn export(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let conn = state.db.get()?;

    let tag_list = tags::list_tags(&conn)?;

    let export_data: Vec<TagExport> = tag_list
        .iter()
        .map(|t| TagExport {
            name: t.name.clone(),
            color: t.color.clone(),
            style: t.style,
        })
        .collect();

    let json = serde_json::to_string_pretty(&export_data)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {}", e)))?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"tags.json\"",
            ),
        ],
        json,
    ))
}

#[derive(Deserialize)]
struct TagImport {
    name: String,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default)]
    style: TagStyle,
}

fn default_color() -> String {
    "#6b7280".to_string()
}

pub async fn import(
    State(state): State<AppState>,
    Json(value): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let data: Vec<TagImport> = serde_json::from_value(value)
        .map_err(|e| AppError::Validation(format!("Invalid JSON format: {}", e)))?;

    let conn = state.db.get()?;

    let existing = tags::list_tags(&conn)?;
    let existing_names: std::collections::HashSet<_> =
        existing.iter().map(|t| t.name.clone()).collect();

    let mut created = 0;
    for item in data {
        if !existing_names.contains(&item.name) {
            let new_tag = NewTag {
                name: item.name,
                color: item.color,
                style: item.style,
            };
            tags::create_tag(&conn, &new_tag)?;
            created += 1;
        }
    }

    Ok(Json(serde_json::json!({
        "imported": created,
        "message": format!("Successfully imported {} tags", created)
    })))
}

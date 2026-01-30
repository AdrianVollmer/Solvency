use askama::Template;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Json, Redirect};
use axum::Form;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::db::queries::categories;
use crate::error::{AppError, AppResult, RenderHtml};
use crate::models::{Category, CategoryWithPath, NewCategory, Settings, TAG_PALETTE};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/categories.html")]
pub struct CategoriesTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub categories: Vec<CategoryWithPath>,
    pub delete_count: i64,
}

#[derive(Template)]
#[template(path = "pages/category_form.html")]
pub struct CategoryFormTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub categories: Vec<CategoryWithPath>,
    pub editing: Option<Category>,
    pub palette: &'static [(&'static str, &'static str)],
}

#[derive(Debug, Deserialize)]
pub struct CategoryFormData {
    pub name: String,
    #[serde(
        default,
        deserialize_with = "crate::form_utils::deserialize_optional_i64"
    )]
    pub parent_id: Option<i64>,
    pub color: Option<String>,
    pub icon: Option<String>,
}

pub async fn new_form(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;
    let cats = categories::list_categories_with_path(&conn)?;

    let template = CategoryFormTemplate {
        title: "Add Category".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        categories: cats,
        editing: None,
        palette: TAG_PALETTE,
    };

    template.render_html()
}

pub async fn edit_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    let conn = state.db.get()?;

    let category = categories::get_category(&conn, id)?
        .ok_or_else(|| AppError::NotFound("Category not found".into()))?;

    if category.built_in {
        return Ok(Redirect::to("/categories").into_response());
    }

    let app_settings = state.load_settings()?;
    let cats = categories::list_categories_with_path(&conn)?;

    let template = CategoryFormTemplate {
        title: "Edit Category".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        categories: cats,
        editing: Some(category),
        palette: TAG_PALETTE,
    };

    Ok(template.render_html()?.into_response())
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = state.load_settings()?;

    let cats = categories::list_categories_with_path(&conn)?;

    let template = CategoriesTemplate {
        title: "Categories".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        delete_count: cats.len() as i64,
        categories: cats,
    };

    template.render_html()
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<CategoryFormData>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    let new_category = NewCategory {
        name: form.name,
        parent_id: form.parent_id,
        color: form.color.unwrap_or_else(|| "#6b7280".into()),
        icon: form.icon.unwrap_or_else(|| "folder".into()),
    };

    categories::create_category(&conn, &new_category)?;

    Ok(Redirect::to("/categories"))
}

/// Walk the ancestor chain of `proposed_parent_id`; if we encounter
/// `category_id` it means setting this parent would create a cycle.
fn check_circular_parent(
    conn: &r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
    category_id: i64,
    proposed_parent_id: Option<i64>,
) -> AppResult<()> {
    let proposed = match proposed_parent_id {
        Some(pid) => pid,
        None => return Ok(()),
    };
    let all_cats = categories::list_categories(conn)?;
    let parent_map: HashMap<i64, Option<i64>> =
        all_cats.iter().map(|c| (c.id, c.parent_id)).collect();
    let mut current = Some(proposed);
    while let Some(cid) = current {
        if cid == category_id {
            return Err(AppError::Validation(
                "Cannot set parent: would create a circular reference".into(),
            ));
        }
        current = parent_map.get(&cid).copied().flatten();
    }
    Ok(())
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<CategoryFormData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let category = categories::get_category(&conn, id)?;
    if category.map(|c| c.built_in).unwrap_or(false) {
        return Err(AppError::Validation(
            "Built-in categories cannot be modified".into(),
        ));
    }

    check_circular_parent(&conn, id, form.parent_id)?;

    let new_category = NewCategory {
        name: form.name,
        parent_id: form.parent_id,
        color: form.color.unwrap_or_else(|| "#6b7280".into()),
        icon: form.icon.unwrap_or_else(|| "folder".into()),
    };

    categories::update_category(&conn, id, &new_category)?;

    Ok(Html(String::new()))
}

pub async fn update_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<CategoryFormData>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    let category = categories::get_category(&conn, id)?;
    if category.map(|c| c.built_in).unwrap_or(false) {
        return Err(AppError::Validation(
            "Built-in categories cannot be modified".into(),
        ));
    }

    check_circular_parent(&conn, id, form.parent_id)?;

    let new_category = NewCategory {
        name: form.name,
        parent_id: form.parent_id,
        color: form.color.unwrap_or_else(|| "#6b7280".into()),
        icon: form.icon.unwrap_or_else(|| "folder".into()),
    };

    categories::update_category(&conn, id, &new_category)?;

    Ok(Redirect::to("/categories"))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let category = categories::get_category(&conn, id)?;
    if category.map(|c| c.built_in).unwrap_or(false) {
        return Err(AppError::Validation(
            "Built-in categories cannot be deleted".into(),
        ));
    }

    categories::delete_category(&conn, id)?;

    Ok(Html(String::new()))
}

pub async fn delete_all(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    categories::delete_all_categories(&conn)?;

    Ok(Html(String::new()))
}

#[derive(Serialize)]
struct CategoryExport {
    name: String,
    parent_name: Option<String>,
    color: String,
    icon: String,
}

pub async fn export(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let conn = state.db.get()?;

    let cats = categories::list_categories(&conn)?;

    // Build a map of id -> name for parent lookups
    let id_to_name: HashMap<i64, String> = cats.iter().map(|c| (c.id, c.name.clone())).collect();

    // Export with parent names instead of IDs for portability
    // Exclude built-in categories from export (they always exist)
    let export_data: Vec<CategoryExport> = cats
        .iter()
        .filter(|c| !c.built_in)
        .map(|c| CategoryExport {
            name: c.name.clone(),
            parent_name: c.parent_id.and_then(|pid| id_to_name.get(&pid).cloned()),
            color: c.color.clone(),
            icon: c.icon.clone(),
        })
        .collect();

    let json = serde_json::to_string_pretty(&export_data)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {}", e)))?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"categories.json\"",
            ),
        ],
        json,
    ))
}

#[derive(Deserialize)]
struct CategoryImport {
    name: String,
    parent_name: Option<String>,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default = "default_icon")]
    icon: String,
}

fn default_color() -> String {
    "#6b7280".to_string()
}

fn default_icon() -> String {
    "folder".to_string()
}

pub async fn import(
    State(state): State<AppState>,
    Json(value): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let data: Vec<CategoryImport> = serde_json::from_value(value)
        .map_err(|e| AppError::Validation(format!("Invalid JSON format: {}", e)))?;

    let conn = state.db.get()?;

    let mut name_to_id: HashMap<String, i64> = HashMap::new();

    // Seed map with existing categories
    let existing = categories::list_categories(&conn)?;
    for cat in &existing {
        name_to_id.insert(cat.name.clone(), cat.id);
    }

    // Iteratively create categories in dependency order: each pass creates
    // items whose parent already exists in name_to_id.  Repeat until no
    // progress is made (remaining items have broken/missing parent refs).
    let mut remaining: Vec<&CategoryImport> = data.iter().collect();
    let mut created = 0;
    loop {
        let mut next_remaining: Vec<&CategoryImport> = Vec::new();
        let mut progress = false;
        for item in remaining {
            if name_to_id.contains_key(&item.name) {
                continue;
            }
            let parent_resolved = match &item.parent_name {
                None => true,
                Some(pn) => name_to_id.contains_key(pn),
            };
            if parent_resolved {
                let parent_id = item
                    .parent_name
                    .as_ref()
                    .and_then(|pn| name_to_id.get(pn).copied());
                let new_cat = NewCategory {
                    name: item.name.clone(),
                    parent_id,
                    color: item.color.clone(),
                    icon: item.icon.clone(),
                };
                let id = categories::create_category(&conn, &new_cat)?;
                name_to_id.insert(item.name.clone(), id);
                created += 1;
                progress = true;
            } else {
                next_remaining.push(item);
            }
        }
        if !progress {
            break;
        }
        remaining = next_remaining;
    }

    Ok(Json(serde_json::json!({
        "imported": created,
        "message": format!("Successfully imported {} categories", created)
    })))
}

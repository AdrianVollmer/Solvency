use askama::Template;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Json};
use axum::Form;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::db::queries::{categories, settings};
use crate::error::{AppError, AppResult};
use crate::models::{CategoryWithPath, NewCategory, Settings};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/categories.html")]
pub struct CategoriesTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub categories: Vec<CategoryWithPath>,
}

#[derive(Template)]
#[template(path = "components/category_row.html")]
pub struct CategoryRowTemplate {
    pub category: CategoryWithPath,
}

#[derive(Debug, Deserialize)]
pub struct CategoryFormData {
    pub name: String,
    pub parent_id: Option<i64>,
    pub color: Option<String>,
    pub icon: Option<String>,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

    let cats = categories::list_categories_with_path(&conn)?;

    let template = CategoriesTemplate {
        title: "Categories".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        version: VERSION,
        categories: cats,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<CategoryFormData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let new_category = NewCategory {
        name: form.name,
        parent_id: form.parent_id,
        color: form.color.unwrap_or_else(|| "#6b7280".into()),
        icon: form.icon.unwrap_or_else(|| "folder".into()),
    };

    let id = categories::create_category(&conn, &new_category)?;

    let cat = categories::get_category(&conn, id)?
        .ok_or_else(|| AppError::Internal("Failed to retrieve created category".into()))?;

    let template = CategoryRowTemplate {
        category: CategoryWithPath {
            category: cat,
            path: new_category.name,
            depth: if new_category.parent_id.is_some() {
                1
            } else {
                0
            },
        },
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<CategoryFormData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let new_category = NewCategory {
        name: form.name,
        parent_id: form.parent_id,
        color: form.color.unwrap_or_else(|| "#6b7280".into()),
        icon: form.icon.unwrap_or_else(|| "folder".into()),
    };

    categories::update_category(&conn, id, &new_category)?;

    let cat = categories::get_category(&conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Category {} not found", id)))?;

    let template = CategoryRowTemplate {
        category: CategoryWithPath {
            category: cat,
            path: new_category.name,
            depth: if new_category.parent_id.is_some() {
                1
            } else {
                0
            },
        },
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    categories::delete_category(&conn, id)?;

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
    let export_data: Vec<CategoryExport> = cats
        .iter()
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

    // First pass: create categories without parents, build name -> id map
    let mut name_to_id: HashMap<String, i64> = HashMap::new();
    let mut deferred: Vec<&CategoryImport> = Vec::new();

    // Get existing categories
    let existing = categories::list_categories(&conn)?;
    for cat in &existing {
        name_to_id.insert(cat.name.clone(), cat.id);
    }

    // Process items - those without parents first, then those with parents
    for item in &data {
        if item.parent_name.is_none() {
            if !name_to_id.contains_key(&item.name) {
                let new_cat = NewCategory {
                    name: item.name.clone(),
                    parent_id: None,
                    color: item.color.clone(),
                    icon: item.icon.clone(),
                };
                let id = categories::create_category(&conn, &new_cat)?;
                name_to_id.insert(item.name.clone(), id);
            }
        } else {
            deferred.push(item);
        }
    }

    // Second pass: create categories with parents
    let mut created = 0;
    for item in deferred {
        if !name_to_id.contains_key(&item.name) {
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
        }
    }

    created += data
        .iter()
        .filter(|i| i.parent_name.is_none() && !existing.iter().any(|e| e.name == i.name))
        .count();

    Ok(Json(serde_json::json!({
        "imported": created,
        "message": format!("Successfully imported {} categories", created)
    })))
}

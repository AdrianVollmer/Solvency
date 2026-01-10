use askama::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::Form;
use serde::Deserialize;

use crate::db::queries::{categories, settings};
use crate::error::{AppError, AppResult};
use crate::models::{CategoryWithPath, NewCategory, Settings};
use crate::state::{AppState, JsManifest};

#[derive(Template)]
#[template(path = "pages/categories.html")]
pub struct CategoriesTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
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

    let settings_map = settings::get_all_settings(&conn)?;
    let app_settings = Settings::from_map(settings_map);

    let cats = categories::list_categories_with_path(&conn)?;

    let template = CategoriesTemplate {
        title: "Categories".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
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

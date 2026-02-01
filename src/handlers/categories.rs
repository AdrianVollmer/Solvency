use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect};
use axum::Form;
use serde::Deserialize;
use std::collections::HashMap;

use crate::db::queries::{categories, transactions};
use crate::error::{AppError, AppResult, RenderHtml};
use crate::models::{Category, CategoryWithPath, NewCategory, Settings, TAG_PALETTE};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

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
    pub prefill: Option<Category>,
    pub palette: &'static [(&'static str, &'static str)],
    pub back_url: String,
}

#[derive(Template)]
#[template(path = "pages/category_detail.html")]
pub struct CategoryDetailTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub category: CategoryWithPath,
    pub transaction_count: i64,
    pub children: Vec<CategoryWithPath>,
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

#[derive(Debug, Deserialize)]
pub struct NewFormQuery {
    pub clone_from: Option<i64>,
}

pub async fn new_form(
    State(state): State<AppState>,
    Query(query): Query<NewFormQuery>,
) -> AppResult<Html<String>> {
    let app_settings = state.load_settings()?;
    let cats = state.cached_categories_with_path()?;

    let prefill = if let Some(id) = query.clone_from {
        let conn = state.db.get()?;
        categories::get_category(&conn, id)?
    } else {
        None
    };

    let template = CategoryFormTemplate {
        title: "Add Category".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        categories: cats,
        editing: None,
        prefill,
        palette: TAG_PALETTE,
        back_url: "/manage?tab=categories".into(),
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
        return Ok(Redirect::to("/manage?tab=categories").into_response());
    }

    let app_settings = state.load_settings()?;
    let cats = state.cached_categories_with_path()?;

    let back_url = format!("/categories/{}", id);
    let template = CategoryFormTemplate {
        title: "Edit Category".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        categories: cats,
        editing: Some(category),
        prefill: None,
        palette: TAG_PALETTE,
        back_url,
    };

    Ok(template.render_html()?.into_response())
}

pub async fn show(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let category = categories::get_category_with_path(&conn, id)?
        .ok_or_else(|| AppError::NotFound("Category not found".into()))?;

    let app_settings = state.load_settings()?;

    let filter = transactions::TransactionFilter {
        category_id: Some(id),
        ..Default::default()
    };
    let transaction_count = transactions::count_transactions(&conn, &filter)?;

    let all_cats = state.cached_categories_with_path()?;
    let children: Vec<CategoryWithPath> = all_cats
        .into_iter()
        .filter(|c| c.category.parent_id == Some(id))
        .collect();

    let template = CategoryDetailTemplate {
        title: category.category.name.clone(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        category,
        transaction_count,
        children,
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

    Ok(Redirect::to("/manage?tab=categories"))
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

    Ok(Redirect::to(&format!("/categories/{}", id)))
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

pub async fn unset_transactions(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    categories::get_category(&conn, id)?
        .ok_or_else(|| AppError::NotFound("Category not found".into()))?;

    transactions::unset_category(&conn, id)?;

    Ok(Html(String::new()))
}

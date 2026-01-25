use askama::Template;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Json, Redirect};
use axum::Form;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::db::queries::{categories, rules, settings, tags};
use crate::error::{AppError, AppResult};
use crate::models::{CategoryWithPath, NewRule, Rule, RuleActionType, Settings, Tag};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/rules.html")]
pub struct RulesTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub rules: Vec<Rule>,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
}

#[derive(Template)]
#[template(path = "pages/rule_form.html")]
pub struct RuleFormTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
}

#[derive(Template)]
#[template(path = "components/rule_row.html")]
pub struct RuleRowTemplate {
    pub icons: crate::filters::Icons,
    pub rule: Rule,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Deserialize)]
pub struct RuleFormData {
    pub name: String,
    pub pattern: String,
    pub action_type: String,
    pub action_value: String,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

    let rule_list = rules::list_rules(&conn)?;
    let category_list = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

    let template = RulesTemplate {
        title: "Rules".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        rules: rule_list,
        categories: category_list,
        tags: tag_list,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn new_form(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = settings::get_settings(&conn)?;
    let category_list = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

    let template = RuleFormTemplate {
        title: "Add Rule".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        categories: category_list,
        tags: tag_list,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<RuleFormData>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    let action_type = RuleActionType::parse(&form.action_type)
        .ok_or_else(|| AppError::Validation("Invalid action type".into()))?;

    let new_rule = NewRule {
        name: form.name,
        pattern: form.pattern,
        action_type,
        action_value: form.action_value,
    };

    rules::create_rule(&conn, &new_rule)?;

    Ok(Redirect::to("/rules"))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<RuleFormData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let action_type = RuleActionType::parse(&form.action_type)
        .ok_or_else(|| AppError::Validation("Invalid action type".into()))?;

    let updated_rule = NewRule {
        name: form.name,
        pattern: form.pattern,
        action_type,
        action_value: form.action_value,
    };

    rules::update_rule(&conn, id, &updated_rule)?;

    let rule =
        rules::get_rule(&conn, id)?.ok_or_else(|| AppError::NotFound("Rule not found".into()))?;

    let category_list = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

    let template = RuleRowTemplate {
        icons: crate::filters::Icons,
        rule,
        categories: category_list,
        tags: tag_list,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    rules::delete_rule(&conn, id)?;

    Ok(Html(String::new()))
}

#[derive(Serialize)]
struct RuleExport {
    name: String,
    pattern: String,
    action_type: RuleActionType,
    action_value: String, // Name of category or tag
}

pub async fn export(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let conn = state.db.get()?;

    let rule_list = rules::list_rules(&conn)?;
    let cat_list = categories::list_categories(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

    // Build maps of id -> name
    let cat_id_to_name: HashMap<String, String> = cat_list
        .iter()
        .map(|c| (c.id.to_string(), c.name.clone()))
        .collect();
    let tag_id_to_name: HashMap<String, String> = tag_list
        .iter()
        .map(|t| (t.id.to_string(), t.name.clone()))
        .collect();

    let export_data: Vec<RuleExport> = rule_list
        .iter()
        .map(|r| {
            let action_value = match r.action_type {
                RuleActionType::AssignCategory => cat_id_to_name
                    .get(&r.action_value)
                    .cloned()
                    .unwrap_or_else(|| r.action_value.clone()),
                RuleActionType::AssignTag => tag_id_to_name
                    .get(&r.action_value)
                    .cloned()
                    .unwrap_or_else(|| r.action_value.clone()),
            };
            RuleExport {
                name: r.name.clone(),
                pattern: r.pattern.clone(),
                action_type: r.action_type,
                action_value,
            }
        })
        .collect();

    let json = serde_json::to_string_pretty(&export_data)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {}", e)))?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"rules.json\"",
            ),
        ],
        json,
    ))
}

#[derive(Deserialize)]
struct RuleImport {
    name: String,
    pattern: String,
    action_type: RuleActionType,
    action_value: String, // Name of category or tag
}

pub async fn import(
    State(state): State<AppState>,
    Json(value): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let data: Vec<RuleImport> = serde_json::from_value(value)
        .map_err(|e| AppError::Validation(format!("Invalid JSON format: {}", e)))?;

    let conn = state.db.get()?;

    let cat_list = categories::list_categories(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

    // Build maps of name -> id
    let cat_name_to_id: HashMap<String, i64> =
        cat_list.iter().map(|c| (c.name.clone(), c.id)).collect();
    let tag_name_to_id: HashMap<String, i64> =
        tag_list.iter().map(|t| (t.name.clone(), t.id)).collect();

    let existing_rules = rules::list_rules(&conn)?;
    let existing_names: std::collections::HashSet<_> =
        existing_rules.iter().map(|r| r.name.clone()).collect();

    let mut created = 0;
    let mut skipped = 0;

    for item in data {
        if existing_names.contains(&item.name) {
            skipped += 1;
            continue;
        }

        // Resolve action_value name to id
        let action_value = match item.action_type {
            RuleActionType::AssignCategory => {
                if let Some(id) = cat_name_to_id.get(&item.action_value) {
                    id.to_string()
                } else {
                    skipped += 1;
                    continue; // Skip if category not found
                }
            }
            RuleActionType::AssignTag => {
                if let Some(id) = tag_name_to_id.get(&item.action_value) {
                    id.to_string()
                } else {
                    skipped += 1;
                    continue; // Skip if tag not found
                }
            }
        };

        let new_rule = NewRule {
            name: item.name,
            pattern: item.pattern,
            action_type: item.action_type,
            action_value,
        };
        rules::create_rule(&conn, &new_rule)?;
        created += 1;
    }

    Ok(Json(serde_json::json!({
        "imported": created,
        "skipped": skipped,
        "message": format!("Imported {} rules, skipped {}", created, skipped)
    })))
}

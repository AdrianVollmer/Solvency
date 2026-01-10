use askama::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::Form;
use serde::Deserialize;

use crate::db::queries::{categories, rules, settings, tags};
use crate::error::{AppError, AppResult};
use crate::models::{CategoryWithPath, NewRule, Rule, RuleActionType, Settings, Tag};
use crate::state::{AppState, JsManifest};

#[derive(Template)]
#[template(path = "pages/rules.html")]
pub struct RulesTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub rules: Vec<Rule>,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
}

#[derive(Template)]
#[template(path = "components/rule_row.html")]
pub struct RuleRowTemplate {
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

    let settings_map = settings::get_all_settings(&conn)?;
    let app_settings = Settings::from_map(settings_map);

    let rule_list = rules::list_rules(&conn)?;
    let category_list = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

    let template = RulesTemplate {
        title: "Rules".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        rules: rule_list,
        categories: category_list,
        tags: tag_list,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<RuleFormData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let action_type = RuleActionType::parse(&form.action_type)
        .ok_or_else(|| AppError::Validation("Invalid action type".into()))?;

    let new_rule = NewRule {
        name: form.name,
        pattern: form.pattern,
        action_type,
        action_value: form.action_value,
    };

    let id = rules::create_rule(&conn, &new_rule)?;

    let rule = rules::get_rule(&conn, id)?
        .ok_or_else(|| AppError::Internal("Failed to retrieve created rule".into()))?;

    let category_list = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

    let template = RuleRowTemplate {
        rule,
        categories: category_list,
        tags: tag_list,
    };

    Ok(Html(template.render().unwrap()))
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

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, Redirect};
use axum::Form;
use regex::RegexBuilder;
use serde::Deserialize;

use crate::db::queries::transactions::TransactionFilter;
use crate::db::queries::{rules, transactions};
use crate::error::{AppError, AppResult, RenderHtml};
use crate::models::{
    CategoryWithPath, NewRule, Rule, RuleActionType, Settings, Tag, TransactionWithRelations,
};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

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
#[template(path = "pages/rule_detail.html")]
pub struct RuleDetailTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub rule: Rule,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
}

#[derive(Template)]
#[template(path = "pages/rule_edit.html")]
pub struct RuleEditTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub rule: Rule,
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

#[derive(Template)]
#[template(path = "pages/rule_preview.html")]
pub struct RulePreviewTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub rule: Rule,
    pub scope: String,
    pub matched: Vec<TransactionWithRelations>,
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

#[derive(Debug, Deserialize)]
pub struct PreviewQuery {
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApplyFormData {
    pub scope: String,
}

pub async fn new_form(State(state): State<AppState>) -> AppResult<Html<String>> {
    let app_settings = state.load_settings()?;
    let category_list = state.cached_categories_with_path()?;
    let tag_list = state.cached_tags()?;

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

    template.render_html()
}

pub async fn detail(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    let rule =
        rules::get_rule(&conn, id)?.ok_or_else(|| AppError::NotFound("Rule not found".into()))?;
    let category_list = state.cached_categories_with_path()?;
    let tag_list = state.cached_tags()?;

    let template = RuleDetailTemplate {
        title: format!("Rule: {}", rule.name),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        rule,
        categories: category_list,
        tags: tag_list,
    };

    template.render_html()
}

pub async fn edit_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    let rule =
        rules::get_rule(&conn, id)?.ok_or_else(|| AppError::NotFound("Rule not found".into()))?;
    let category_list = state.cached_categories_with_path()?;
    let tag_list = state.cached_tags()?;

    let template = RuleEditTemplate {
        title: format!("Edit Rule: {}", rule.name),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        rule,
        categories: category_list,
        tags: tag_list,
    };

    template.render_html()
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

    Ok(Redirect::to("/manage?tab=rules"))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<RuleFormData>,
) -> AppResult<Redirect> {
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

    Ok(Redirect::to(&format!("/rules/{id}")))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    rules::delete_rule(&conn, id)?;

    Ok(Html(String::new()))
}

pub async fn delete_all(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    rules::delete_all_rules(&conn)?;

    Ok(Html(String::new()))
}

/// Fetch transactions matching a rule's regex pattern, filtered by scope.
fn match_transactions(
    conn: &rusqlite::Connection,
    rule: &Rule,
    scope: &str,
) -> AppResult<Vec<TransactionWithRelations>> {
    let re = RegexBuilder::new(&rule.pattern)
        .case_insensitive(true)
        .build()
        .map_err(|e| AppError::Validation(format!("Invalid regex pattern: {e}")))?;

    let filter = TransactionFilter {
        uncategorized_only: scope == "uncategorized",
        ..Default::default()
    };
    let all = transactions::list_transactions(conn, &filter)?;

    let matched: Vec<TransactionWithRelations> = all
        .into_iter()
        .filter(|t| re.is_match(&t.transaction.description))
        .collect();

    Ok(matched)
}

pub async fn preview(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<PreviewQuery>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    let rule =
        rules::get_rule(&conn, id)?.ok_or_else(|| AppError::NotFound("Rule not found".into()))?;

    let scope = params.scope.unwrap_or_else(|| "all".into());
    let matched = match_transactions(&conn, &rule, &scope)?;
    let category_list = state.cached_categories_with_path()?;
    let tag_list = state.cached_tags()?;

    let template = RulePreviewTemplate {
        title: format!("Preview: {}", rule.name),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        rule,
        scope,
        matched,
        categories: category_list,
        tags: tag_list,
    };

    template.render_html()
}

pub async fn apply(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<ApplyFormData>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    let rule =
        rules::get_rule(&conn, id)?.ok_or_else(|| AppError::NotFound("Rule not found".into()))?;

    let matched = match_transactions(&conn, &rule, &form.scope)?;
    let ids: Vec<i64> = matched.iter().map(|t| t.transaction.id).collect();

    match rule.action_type {
        RuleActionType::AssignCategory => {
            let category_id: i64 = rule
                .action_value
                .parse()
                .map_err(|_| AppError::Validation("Invalid category ID".into()))?;
            rules::apply_rule_category(&conn, &ids, category_id)?;
        }
        RuleActionType::AssignTag => {
            let tag_id: i64 = rule
                .action_value
                .parse()
                .map_err(|_| AppError::Validation("Invalid tag ID".into()))?;
            rules::apply_rule_tag(&conn, &ids, tag_id)?;
        }
    }

    Ok(Redirect::to(&format!("/rules/{id}")))
}

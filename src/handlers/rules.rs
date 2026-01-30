use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Json, Redirect};
use axum::Form;
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::db::queries::transactions::TransactionFilter;
use crate::db::queries::{categories, rules, tags, transactions};
use crate::error::{AppError, AppResult, RenderHtml};
use crate::handlers::import_preview::{
    ImportPreviewForm, ImportPreviewItem, ImportPreviewStatus, ImportPreviewTemplate,
};
use crate::models::{
    CategoryWithPath, NewRule, Rule, RuleActionType, Settings, Tag, TransactionWithRelations,
};
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
    pub delete_count: i64,
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

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = state.load_settings()?;

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
        delete_count: rule_list.len() as i64,
        rules: rule_list,
        categories: category_list,
        tags: tag_list,
    };

    template.render_html()
}

pub async fn new_form(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;
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

    template.render_html()
}

pub async fn detail(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    let rule =
        rules::get_rule(&conn, id)?.ok_or_else(|| AppError::NotFound("Rule not found".into()))?;
    let category_list = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

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
    let category_list = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

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

    Ok(Redirect::to("/rules"))
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
    let mut errors: Vec<String> = Vec::new();

    for item in data {
        if existing_names.contains(&item.name) {
            skipped += 1;
            errors.push(format!(
                "\"{}\": a rule with this name already exists",
                item.name
            ));
            continue;
        }

        // Resolve action_value name to id
        let action_value = match item.action_type {
            RuleActionType::AssignCategory => {
                if let Some(id) = cat_name_to_id.get(&item.action_value) {
                    id.to_string()
                } else {
                    skipped += 1;
                    errors.push(format!(
                        "\"{}\": category \"{}\" not found",
                        item.name, item.action_value
                    ));
                    continue;
                }
            }
            RuleActionType::AssignTag => {
                if let Some(id) = tag_name_to_id.get(&item.action_value) {
                    id.to_string()
                } else {
                    skipped += 1;
                    errors.push(format!(
                        "\"{}\": tag \"{}\" not found",
                        item.name, item.action_value
                    ));
                    continue;
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
        "errors": errors,
        "message": format!("Imported {} rules, skipped {}", created, skipped)
    })))
}

pub async fn import_preview(
    State(state): State<AppState>,
    Form(form): Form<ImportPreviewForm>,
) -> AppResult<Html<String>> {
    let data: Vec<RuleImport> = serde_json::from_str(&form.data)
        .map_err(|e| AppError::Validation(format!("Invalid JSON format: {}", e)))?;

    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    let cat_list = categories::list_categories(&conn)?;
    let tag_list = tags::list_tags(&conn)?;
    let cat_names: std::collections::HashSet<String> =
        cat_list.iter().map(|c| c.name.clone()).collect();
    let tag_names: std::collections::HashSet<String> =
        tag_list.iter().map(|t| t.name.clone()).collect();

    let existing_rules = rules::list_rules(&conn)?;
    let existing_names: std::collections::HashSet<String> =
        existing_rules.iter().map(|r| r.name.clone()).collect();

    let mut items = Vec::new();
    let mut ok_count = 0;
    let mut skip_count = 0;

    for item in &data {
        let action_label = match item.action_type {
            RuleActionType::AssignCategory => "Assign Category",
            RuleActionType::AssignTag => "Assign Tag",
        };
        let cells = vec![
            item.name.clone(),
            item.pattern.clone(),
            action_label.to_string(),
            item.action_value.clone(),
        ];

        if existing_names.contains(&item.name) {
            skip_count += 1;
            items.push(ImportPreviewItem {
                status: ImportPreviewStatus::Skipped,
                reason: "already exists".to_string(),
                cells,
            });
        } else {
            let target_exists = match item.action_type {
                RuleActionType::AssignCategory => cat_names.contains(&item.action_value),
                RuleActionType::AssignTag => tag_names.contains(&item.action_value),
            };
            if !target_exists {
                let kind = match item.action_type {
                    RuleActionType::AssignCategory => "category",
                    RuleActionType::AssignTag => "tag",
                };
                skip_count += 1;
                items.push(ImportPreviewItem {
                    status: ImportPreviewStatus::Skipped,
                    reason: format!("{} \"{}\" not found", kind, item.action_value),
                    cells,
                });
            } else {
                ok_count += 1;
                items.push(ImportPreviewItem {
                    status: ImportPreviewStatus::Ok,
                    reason: String::new(),
                    cells,
                });
            }
        }
    }

    let template = ImportPreviewTemplate {
        title: "Import Rules — Preview".to_string(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        resource_name: "Rules".to_string(),
        back_url: "/rules".to_string(),
        import_url: "/rules/import".to_string(),
        columns: vec![
            "Name".to_string(),
            "Pattern".to_string(),
            "Action".to_string(),
            "Target".to_string(),
        ],
        items,
        ok_count,
        skip_count,
        raw_json: form.data,
    };

    template.render_html()
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
    let category_list = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

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

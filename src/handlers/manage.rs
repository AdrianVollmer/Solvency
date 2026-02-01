use askama::Template;
use axum::extract::State;
use axum::http::header;
use axum::response::{Html, IntoResponse, Json};
use axum::Form;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::db::queries::{rules, tags};
use crate::error::{AppError, AppResult, RenderHtml};
use crate::handlers::import_preview::{ImportPreviewForm, ImportPreviewItem, ImportPreviewStatus};
use crate::models::{
    CategoryWithPath, NewCategory, NewRule, NewTag, Rule, RuleActionType, Settings, Tag, TagStyle,
    TagWithUsage, TAG_PALETTE,
};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Debug, Deserialize)]
pub struct ManageQuery {
    pub tab: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/manage.html")]
pub struct ManageTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub active_tab: String,
    pub categories: Vec<CategoryWithPath>,
    pub tags_with_usage: Vec<TagWithUsage>,
    pub tags: Vec<Tag>,
    pub rules: Vec<Rule>,
    pub category_count: i64,
    pub tag_count: i64,
    pub rule_count: i64,
    pub palette: &'static [(&'static str, &'static str)],
}

pub async fn index(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<ManageQuery>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    let active_tab = params.tab.unwrap_or_else(|| "categories".into());

    let categories = state.cached_categories_with_path()?;
    let tags_with_usage = tags::list_tags_with_usage(&conn)?;
    let tag_list = state.cached_tags()?;
    let rule_list = rules::list_rules(&conn)?;

    let category_count = categories.len() as i64;
    let tag_count = tags_with_usage.len() as i64;
    let rule_count = rule_list.len() as i64;

    let template = ManageTemplate {
        title: "Manage".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        active_tab,
        categories,
        tags_with_usage,
        tags: tag_list,
        rules: rule_list,
        category_count,
        tag_count,
        rule_count,
        palette: TAG_PALETTE,
    };

    template.render_html()
}

// -- Unified export --

#[derive(Serialize)]
struct CategoryExport {
    name: String,
    parent_name: Option<String>,
    color: String,
    icon: String,
}

#[derive(Serialize)]
struct TagExport {
    name: String,
    color: String,
    style: TagStyle,
}

#[derive(Serialize)]
struct RuleExport {
    name: String,
    pattern: String,
    action_type: RuleActionType,
    action_value: String,
}

#[derive(Serialize)]
struct ExportEnvelope {
    header: ExportHeader,
    body: ExportBody,
}

#[derive(Serialize)]
struct ExportHeader {
    version: String,
    #[serde(rename = "type")]
    export_type: String,
}

#[derive(Serialize)]
struct ExportBody {
    categories: Vec<CategoryExport>,
    tags: Vec<TagExport>,
    rules: Vec<RuleExport>,
}

pub async fn export(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let conn = state.db.get()?;

    let cats = state.cached_categories()?;
    let id_to_name: HashMap<i64, String> = cats.iter().map(|c| (c.id, c.name.clone())).collect();

    let categories: Vec<CategoryExport> = cats
        .iter()
        .filter(|c| !c.built_in)
        .map(|c| CategoryExport {
            name: c.name.clone(),
            parent_name: c.parent_id.and_then(|pid| id_to_name.get(&pid).cloned()),
            color: c.color.clone(),
            icon: c.icon.clone(),
        })
        .collect();

    let tag_list = state.cached_tags()?;
    let tags: Vec<TagExport> = tag_list
        .iter()
        .map(|t| TagExport {
            name: t.name.clone(),
            color: t.color.clone(),
            style: t.style,
        })
        .collect();

    let rule_list = rules::list_rules(&conn)?;
    let cat_id_to_name: HashMap<String, String> = cats
        .iter()
        .map(|c| (c.id.to_string(), c.name.clone()))
        .collect();
    let tag_id_to_name: HashMap<String, String> = tag_list
        .iter()
        .map(|t| (t.id.to_string(), t.name.clone()))
        .collect();

    let rules: Vec<RuleExport> = rule_list
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

    let envelope = ExportEnvelope {
        header: ExportHeader {
            version: VERSION.to_string(),
            export_type: "solvency-manage-export".to_string(),
        },
        body: ExportBody {
            categories,
            tags,
            rules,
        },
    };

    tracing::info!("Exporting manage data (unified)");

    let json = serde_json::to_string_pretty(&envelope)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {}", e)))?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"solvency-manage.json\"",
            ),
        ],
        json,
    ))
}

// -- Unified import --

#[derive(Deserialize)]
struct ImportEnvelope {
    header: ImportHeader,
    body: ImportBody,
}

#[derive(Deserialize)]
struct ImportHeader {
    #[serde(rename = "type")]
    export_type: String,
}

#[derive(Deserialize)]
struct ImportBody {
    #[serde(default)]
    categories: Vec<CategoryImport>,
    #[serde(default)]
    tags: Vec<TagImport>,
    #[serde(default)]
    rules: Vec<RuleImport>,
}

#[derive(Deserialize, Clone)]
struct CategoryImport {
    name: String,
    parent_name: Option<String>,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default = "default_icon")]
    icon: String,
}

#[derive(Deserialize, Clone)]
struct TagImport {
    name: String,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default)]
    style: TagStyle,
}

#[derive(Deserialize, Clone)]
struct RuleImport {
    name: String,
    pattern: String,
    action_type: RuleActionType,
    action_value: String,
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
    let envelope: ImportEnvelope = serde_json::from_value(value)
        .map_err(|e| AppError::Validation(format!("Invalid JSON format: {}", e)))?;

    if envelope.header.export_type != "solvency-manage-export" {
        return Err(AppError::Validation(
            "Invalid export file: expected type \"solvency-manage-export\"".into(),
        ));
    }

    let conn = state.db.get()?;
    let mut errors: Vec<String> = Vec::new();

    // 1. Import categories (parents before children, iterative passes)
    let mut name_to_id: HashMap<String, i64> = HashMap::new();
    let existing_cats = state.cached_categories()?;
    for cat in &existing_cats {
        name_to_id.insert(cat.name.clone(), cat.id);
    }

    let mut remaining: Vec<&CategoryImport> = envelope.body.categories.iter().collect();
    let mut categories_created = 0;
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
                match crate::db::queries::categories::create_category(&conn, &new_cat) {
                    Ok(id) => {
                        name_to_id.insert(item.name.clone(), id);
                        categories_created += 1;
                        progress = true;
                    }
                    Err(e) => {
                        errors.push(format!("category \"{}\": {}", item.name, e));
                    }
                }
            } else {
                next_remaining.push(item);
            }
        }
        if !progress {
            for item in &next_remaining {
                errors.push(format!(
                    "category \"{}\": parent \"{}\" not found",
                    item.name,
                    item.parent_name.as_deref().unwrap_or("?")
                ));
            }
            break;
        }
        remaining = next_remaining;
    }

    // 2. Import tags
    let fresh_tags = tags::list_tags(&conn)?;
    let existing_tag_names: std::collections::HashSet<String> =
        fresh_tags.iter().map(|t| t.name.clone()).collect();

    let mut tags_created = 0;
    for item in &envelope.body.tags {
        if existing_tag_names.contains(&item.name) {
            continue;
        }
        let new_tag = NewTag {
            name: item.name.clone(),
            color: item.color.clone(),
            style: item.style,
        };
        match tags::create_tag(&conn, &new_tag) {
            Ok(_) => tags_created += 1,
            Err(e) => errors.push(format!("tag \"{}\": {}", item.name, e)),
        }
    }

    // 3. Import rules (resolve category/tag names to IDs from fresh data)
    let fresh_cats = crate::db::queries::categories::list_categories(&conn)?;
    let cat_name_to_id: HashMap<String, i64> =
        fresh_cats.iter().map(|c| (c.name.clone(), c.id)).collect();
    let fresh_tags_2 = tags::list_tags(&conn)?;
    let tag_name_to_id: HashMap<String, i64> = fresh_tags_2
        .iter()
        .map(|t| (t.name.clone(), t.id))
        .collect();

    let existing_rules = rules::list_rules(&conn)?;
    let existing_rule_names: std::collections::HashSet<String> =
        existing_rules.iter().map(|r| r.name.clone()).collect();

    let mut rules_created = 0;
    for item in &envelope.body.rules {
        if existing_rule_names.contains(&item.name) {
            continue;
        }

        let action_value = match item.action_type {
            RuleActionType::AssignCategory => {
                if let Some(id) = cat_name_to_id.get(&item.action_value) {
                    id.to_string()
                } else {
                    errors.push(format!(
                        "rule \"{}\": category \"{}\" not found",
                        item.name, item.action_value
                    ));
                    continue;
                }
            }
            RuleActionType::AssignTag => {
                if let Some(id) = tag_name_to_id.get(&item.action_value) {
                    id.to_string()
                } else {
                    errors.push(format!(
                        "rule \"{}\": tag \"{}\" not found",
                        item.name, item.action_value
                    ));
                    continue;
                }
            }
        };

        let new_rule = NewRule {
            name: item.name.clone(),
            pattern: item.pattern.clone(),
            action_type: item.action_type,
            action_value,
        };
        match rules::create_rule(&conn, &new_rule) {
            Ok(_) => rules_created += 1,
            Err(e) => errors.push(format!("rule \"{}\": {}", item.name, e)),
        }
    }

    tracing::info!(
        categories = categories_created,
        tags = tags_created,
        rules = rules_created,
        errors = errors.len(),
        "Manage import completed"
    );

    let message = format!(
        "Imported {} categories, {} tags, {} rules{}",
        categories_created,
        tags_created,
        rules_created,
        if errors.is_empty() {
            String::new()
        } else {
            format!(" ({} errors)", errors.len())
        }
    );

    Ok(Json(serde_json::json!({
        "imported": {
            "categories": categories_created,
            "tags": tags_created,
            "rules": rules_created
        },
        "errors": errors,
        "message": message
    })))
}

// -- Unified import preview --

#[derive(Template)]
#[template(path = "pages/manage_import_preview.html")]
pub struct ManageImportPreviewTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub category_items: Vec<ImportPreviewItem>,
    pub tag_items: Vec<ImportPreviewItem>,
    pub rule_items: Vec<ImportPreviewItem>,
    pub category_ok: usize,
    pub category_skip: usize,
    pub tag_ok: usize,
    pub tag_skip: usize,
    pub rule_ok: usize,
    pub rule_skip: usize,
    pub total_ok: usize,
    pub raw_json: String,
}

pub async fn import_preview(
    State(state): State<AppState>,
    Form(form): Form<ImportPreviewForm>,
) -> AppResult<Html<String>> {
    let envelope: ImportEnvelope = serde_json::from_str(&form.data)
        .map_err(|e| AppError::Validation(format!("Invalid JSON format: {}", e)))?;

    if envelope.header.export_type != "solvency-manage-export" {
        return Err(AppError::Validation(
            "Invalid export file: expected type \"solvency-manage-export\"".into(),
        ));
    }

    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    // Categories preview
    let existing_cats = state.cached_categories()?;
    let existing_cat_names: std::collections::HashSet<String> =
        existing_cats.iter().map(|c| c.name.clone()).collect();
    let all_cat_names: std::collections::HashSet<String> = existing_cat_names
        .iter()
        .cloned()
        .chain(envelope.body.categories.iter().map(|c| c.name.clone()))
        .collect();

    let mut category_items = Vec::new();
    let mut category_ok = 0;
    let mut category_skip = 0;

    for item in &envelope.body.categories {
        let cells = vec![
            item.name.clone(),
            item.parent_name.clone().unwrap_or_default(),
            item.color.clone(),
            item.icon.clone(),
        ];

        if existing_cat_names.contains(&item.name) {
            category_skip += 1;
            category_items.push(ImportPreviewItem {
                status: ImportPreviewStatus::Skipped,
                reason: "already exists".to_string(),
                cells,
            });
        } else if let Some(ref pn) = item.parent_name {
            if !all_cat_names.contains(pn) {
                category_skip += 1;
                category_items.push(ImportPreviewItem {
                    status: ImportPreviewStatus::Skipped,
                    reason: format!("parent \"{}\" not found", pn),
                    cells,
                });
            } else {
                category_ok += 1;
                category_items.push(ImportPreviewItem {
                    status: ImportPreviewStatus::Ok,
                    reason: String::new(),
                    cells,
                });
            }
        } else {
            category_ok += 1;
            category_items.push(ImportPreviewItem {
                status: ImportPreviewStatus::Ok,
                reason: String::new(),
                cells,
            });
        }
    }

    // Tags preview
    let existing_tags = state.cached_tags()?;
    let existing_tag_names: std::collections::HashSet<String> =
        existing_tags.iter().map(|t| t.name.clone()).collect();

    let mut tag_items = Vec::new();
    let mut tag_ok = 0;
    let mut tag_skip = 0;

    for item in &envelope.body.tags {
        let cells = vec![
            item.name.clone(),
            item.color.clone(),
            item.style.label().to_string(),
        ];

        if existing_tag_names.contains(&item.name) {
            tag_skip += 1;
            tag_items.push(ImportPreviewItem {
                status: ImportPreviewStatus::Skipped,
                reason: "already exists".to_string(),
                cells,
            });
        } else {
            tag_ok += 1;
            tag_items.push(ImportPreviewItem {
                status: ImportPreviewStatus::Ok,
                reason: String::new(),
                cells,
            });
        }
    }

    // Rules preview
    let cat_names: std::collections::HashSet<String> =
        existing_cats.iter().map(|c| c.name.clone()).collect();
    // Include just-imported category names
    let all_resolve_cat_names: std::collections::HashSet<String> = cat_names
        .iter()
        .cloned()
        .chain(envelope.body.categories.iter().map(|c| c.name.clone()))
        .collect();
    let tag_names: std::collections::HashSet<String> =
        existing_tags.iter().map(|t| t.name.clone()).collect();
    let all_resolve_tag_names: std::collections::HashSet<String> = tag_names
        .iter()
        .cloned()
        .chain(envelope.body.tags.iter().map(|t| t.name.clone()))
        .collect();

    let existing_rules = rules::list_rules(&conn)?;
    let existing_rule_names: std::collections::HashSet<String> =
        existing_rules.iter().map(|r| r.name.clone()).collect();

    let mut rule_items = Vec::new();
    let mut rule_ok = 0;
    let mut rule_skip = 0;

    for item in &envelope.body.rules {
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

        if existing_rule_names.contains(&item.name) {
            rule_skip += 1;
            rule_items.push(ImportPreviewItem {
                status: ImportPreviewStatus::Skipped,
                reason: "already exists".to_string(),
                cells,
            });
        } else {
            let target_exists = match item.action_type {
                RuleActionType::AssignCategory => {
                    all_resolve_cat_names.contains(&item.action_value)
                }
                RuleActionType::AssignTag => all_resolve_tag_names.contains(&item.action_value),
            };
            if !target_exists {
                let kind = match item.action_type {
                    RuleActionType::AssignCategory => "category",
                    RuleActionType::AssignTag => "tag",
                };
                rule_skip += 1;
                rule_items.push(ImportPreviewItem {
                    status: ImportPreviewStatus::Skipped,
                    reason: format!("{} \"{}\" not found", kind, item.action_value),
                    cells,
                });
            } else {
                rule_ok += 1;
                rule_items.push(ImportPreviewItem {
                    status: ImportPreviewStatus::Ok,
                    reason: String::new(),
                    cells,
                });
            }
        }
    }

    let total_ok = category_ok + tag_ok + rule_ok;

    let template = ManageImportPreviewTemplate {
        title: "Import Manage Data — Preview".to_string(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        category_items,
        tag_items,
        rule_items,
        category_ok,
        category_skip,
        tag_ok,
        tag_skip,
        rule_ok,
        rule_skip,
        total_ok,
        raw_json: form.data,
    };

    template.render_html()
}

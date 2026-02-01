use askama::Template;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Json, Redirect};
use axum::Form;
use serde::{Deserialize, Serialize};

use crate::db::queries::accounts;
use crate::error::{AppError, AppResult, RenderHtml};
use crate::handlers::import_preview::{
    ImportPreviewForm, ImportPreviewItem, ImportPreviewStatus, ImportPreviewTemplate,
};
use crate::models::{Account, AccountType, NewAccount, Settings};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

#[derive(Template)]
#[template(path = "pages/accounts.html")]
pub struct AccountsTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub accounts: Vec<Account>,
    pub delete_count: i64,
}

#[derive(Template)]
#[template(path = "pages/account_form.html")]
pub struct AccountFormTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub account: Option<Account>,
}

#[derive(Debug, Deserialize)]
pub struct AccountFormData {
    pub name: String,
    pub account_type: String,
    /// HTML checkbox: "on" when checked, absent (defaults to "") when unchecked.
    #[serde(default)]
    pub active: String,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let app_settings = state.load_settings()?;
    let account_list = state.cached_accounts()?;

    let template = AccountsTemplate {
        title: "Accounts".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        delete_count: account_list.len() as i64,
        accounts: account_list,
    };

    template.render_html()
}

pub async fn new_form(State(state): State<AppState>) -> AppResult<Html<String>> {
    let app_settings = state.load_settings()?;

    let template = AccountFormTemplate {
        title: "Add Account".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        account: None,
    };

    template.render_html()
}

pub async fn edit_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = state.load_settings()?;

    let account = accounts::get_account(&conn, id)?
        .ok_or_else(|| AppError::NotFound("Account not found".into()))?;

    let template = AccountFormTemplate {
        title: "Edit Account".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        account: Some(account),
    };

    template.render_html()
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<AccountFormData>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    let account_type = AccountType::parse(&form.account_type)
        .ok_or_else(|| AppError::Validation("Invalid account type".into()))?;

    let new_account = NewAccount {
        name: form.name,
        account_type,
        active: form.active == "on",
    };

    accounts::create_account(&conn, &new_account)?;

    Ok(Redirect::to("/accounts"))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<AccountFormData>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    let account_type = AccountType::parse(&form.account_type)
        .ok_or_else(|| AppError::Validation("Invalid account type".into()))?;

    let updated_account = NewAccount {
        name: form.name,
        account_type,
        active: form.active == "on",
    };

    accounts::update_account(&conn, id, &updated_account)?;

    Ok(Redirect::to("/accounts"))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    accounts::delete_account(&conn, id)?;

    Ok(Html(String::new()))
}

pub async fn delete_all(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    accounts::delete_all_accounts(&conn)?;

    Ok(Html(String::new()))
}

#[derive(Serialize)]
struct AccountExport {
    name: String,
    account_type: AccountType,
}

pub async fn export(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let account_list = state.cached_accounts()?;

    let export_data: Vec<AccountExport> = account_list
        .iter()
        .map(|a| AccountExport {
            name: a.name.clone(),
            account_type: a.account_type,
        })
        .collect();

    let json = serde_json::to_string_pretty(&export_data)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {}", e)))?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"accounts.json\"",
            ),
        ],
        json,
    ))
}

#[derive(Deserialize)]
struct AccountImport {
    name: String,
    account_type: AccountType,
}

pub async fn import(
    State(state): State<AppState>,
    Json(value): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let data: Vec<AccountImport> = serde_json::from_value(value)
        .map_err(|e| AppError::Validation(format!("Invalid JSON format: {}", e)))?;

    let conn = state.db.get()?;

    let existing = state.cached_accounts()?;
    let existing_names: std::collections::HashSet<_> =
        existing.iter().map(|a| a.name.clone()).collect();

    let mut created = 0;
    for item in data {
        if !existing_names.contains(&item.name) {
            let new_account = NewAccount {
                name: item.name,
                account_type: item.account_type,
                active: true,
            };
            accounts::create_account(&conn, &new_account)?;
            created += 1;
        }
    }

    Ok(Json(serde_json::json!({
        "imported": created,
        "message": format!("Successfully imported {} accounts", created)
    })))
}

pub async fn import_preview(
    State(state): State<AppState>,
    Form(form): Form<ImportPreviewForm>,
) -> AppResult<Html<String>> {
    let data: Vec<AccountImport> = serde_json::from_str(&form.data)
        .map_err(|e| AppError::Validation(format!("Invalid JSON format: {}", e)))?;

    let app_settings = state.load_settings()?;

    let existing = state.cached_accounts()?;
    let existing_names: std::collections::HashSet<String> =
        existing.iter().map(|a| a.name.clone()).collect();

    let mut items = Vec::new();
    let mut ok_count = 0;
    let mut skip_count = 0;

    for item in &data {
        let cells = vec![item.name.clone(), item.account_type.as_str().to_string()];

        if existing_names.contains(&item.name) {
            skip_count += 1;
            items.push(ImportPreviewItem {
                status: ImportPreviewStatus::Skipped,
                reason: "already exists".to_string(),
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

    let template = ImportPreviewTemplate {
        title: "Import Accounts — Preview".to_string(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        resource_name: "Accounts".to_string(),
        back_url: "/accounts".to_string(),
        import_url: "/accounts/import".to_string(),
        columns: vec!["Name".to_string(), "Type".to_string()],
        items,
        ok_count,
        skip_count,
        raw_json: form.data,
    };

    template.render_html()
}

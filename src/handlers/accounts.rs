use askama::Template;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Json, Redirect};
use axum::Form;
use serde::{Deserialize, Serialize};

use crate::db::queries::{accounts, settings};
use crate::error::{AppError, AppResult, RenderHtml};
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
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;
    let account_list = accounts::list_accounts(&conn)?;

    let template = AccountsTemplate {
        title: "Accounts".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        accounts: account_list,
    };

    template.render_html()
}

pub async fn new_form(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = settings::get_settings(&conn)?;

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
    let app_settings = settings::get_settings(&conn)?;

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
    let conn = state.db.get()?;

    let account_list = accounts::list_accounts(&conn)?;

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

    let existing = accounts::list_accounts(&conn)?;
    let existing_names: std::collections::HashSet<_> =
        existing.iter().map(|a| a.name.clone()).collect();

    let mut created = 0;
    for item in data {
        if !existing_names.contains(&item.name) {
            let new_account = NewAccount {
                name: item.name,
                account_type: item.account_type,
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

use askama::Template;
use axum::extract::{Multipart, State};
use axum::response::Html;
use axum::Form;
use serde::Deserialize;

use crate::db::queries::{categories, expenses, settings, tags};
use crate::error::{AppError, AppResult};
use crate::models::{CategoryWithPath, NewExpense, Settings};
use crate::services::csv_parser::{parse_csv, ParsedExpense};
use crate::state::{AppState, JsManifest};

#[derive(Template)]
#[template(path = "pages/import.html")]
pub struct ImportTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub categories: Vec<CategoryWithPath>,
}

#[derive(Template)]
#[template(path = "partials/import_preview.html")]
pub struct ImportPreviewTemplate {
    pub expenses: Vec<ParsedExpense>,
    pub errors: Vec<String>,
    pub categories: Vec<CategoryWithPath>,
}

#[derive(Template)]
#[template(path = "partials/import_result.html")]
pub struct ImportResultTemplate {
    pub imported_count: usize,
    pub error_count: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImportConfirmData {
    #[serde(default)]
    pub expenses: Vec<ImportExpenseData>,
}

#[derive(Debug, Deserialize)]
pub struct ImportExpenseData {
    pub date: String,
    pub amount: String,
    pub currency: String,
    pub description: String,
    pub category_id: Option<i64>,
    #[serde(default)]
    pub tag_names: String,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let settings_map = settings::get_all_settings(&conn)?;
    let app_settings = Settings::from_map(settings_map);
    let cats = categories::list_categories_with_path(&conn)?;

    let template = ImportTemplate {
        title: "Import".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        categories: cats,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let mut csv_content = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::CsvParse(e.to_string()))?
    {
        if field.name() == Some("file") {
            csv_content = field
                .bytes()
                .await
                .map_err(|e| AppError::CsvParse(e.to_string()))?
                .to_vec();
        }
    }

    if csv_content.is_empty() {
        return Err(AppError::Validation("No file uploaded".into()));
    }

    let result = parse_csv(&csv_content)?;
    let cats = categories::list_categories_with_path(&conn)?;

    let template = ImportPreviewTemplate {
        expenses: result.expenses,
        errors: result.errors,
        categories: cats,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn preview(
    State(state): State<AppState>,
    multipart: Multipart,
) -> AppResult<Html<String>> {
    upload(State(state), multipart).await
}

pub async fn confirm(
    State(state): State<AppState>,
    Form(form): Form<ImportConfirmData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let mut imported_count = 0;
    let mut error_count = 0;
    let mut errors = Vec::new();

    for expense_data in form.expenses {
        let amount: f64 = match expense_data.amount.parse() {
            Ok(a) => a,
            Err(_) => {
                error_count += 1;
                errors.push(format!("Invalid amount: {}", expense_data.amount));
                continue;
            }
        };

        let tag_ids: Vec<i64> = if expense_data.tag_names.is_empty() {
            Vec::new()
        } else {
            expense_data
                .tag_names
                .split(',')
                .filter_map(|name| {
                    let name = name.trim();
                    if name.is_empty() {
                        return None;
                    }
                    tags::create_or_get_tag(&conn, name).ok().map(|t| t.id)
                })
                .collect()
        };

        let new_expense = NewExpense {
            date: expense_data.date,
            amount_cents: (amount * 100.0).round() as i64,
            currency: expense_data.currency,
            description: expense_data.description,
            category_id: expense_data.category_id,
            notes: None,
            tag_ids,
            value_date: None,
            payer: None,
            payee: None,
            reference: None,
            transaction_type: None,
            counterparty_iban: None,
            creditor_id: None,
            mandate_reference: None,
            customer_reference: None,
        };

        match expenses::create_expense(&conn, &new_expense) {
            Ok(_) => imported_count += 1,
            Err(e) => {
                error_count += 1;
                errors.push(format!("Failed to import: {}", e));
            }
        }
    }

    let template = ImportResultTemplate {
        imported_count,
        error_count,
        errors,
    };

    Ok(Html(template.render().unwrap()))
}

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::Form;
use serde::Deserialize;

use crate::db::queries::{categories, expenses, settings, tags};
use crate::error::{AppError, AppResult};
use crate::models::{CategoryWithPath, ExpenseWithRelations, NewExpense, Settings, Tag};
use crate::state::{AppState, JsManifest};

#[derive(Template)]
#[template(path = "pages/expenses.html")]
pub struct ExpensesTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub expenses: Vec<ExpenseWithRelations>,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
    pub total_count: i64,
    pub page: i64,
    pub page_size: i64,
    pub filter: ExpenseFilterParams,
}

#[derive(Template)]
#[template(path = "partials/expense_table.html")]
pub struct ExpenseTableTemplate {
    pub expenses: Vec<ExpenseWithRelations>,
    pub total_count: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Template)]
#[template(path = "components/expense_form.html")]
pub struct ExpenseFormTemplate {
    pub expense: Option<ExpenseWithRelations>,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
    pub is_edit: bool,
}

#[derive(Template)]
#[template(path = "components/expense_row.html")]
pub struct ExpenseRowTemplate {
    pub expense: ExpenseWithRelations,
}

#[derive(Template)]
#[template(path = "pages/expense_detail.html")]
pub struct ExpenseDetailTemplate {
    pub title: String,
    pub settings: Settings,
    pub manifest: JsManifest,
    pub expense: ExpenseWithRelations,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ExpenseFilterParams {
    pub search: Option<String>,
    pub category_id: Option<i64>,
    pub tag_id: Option<i64>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub page: Option<i64>,
}

impl ExpenseFilterParams {
    pub fn matches_category(&self, id: &i64) -> bool {
        self.category_id == Some(*id)
    }
}

#[derive(Debug, Deserialize)]
pub struct ExpenseFormData {
    pub date: String,
    pub amount: String,
    pub currency: String,
    pub description: String,
    pub category_id: Option<i64>,
    pub notes: Option<String>,
    #[serde(default)]
    pub tag_ids: Vec<i64>,
}

impl ExpenseFormData {
    fn to_new_expense(&self) -> Result<NewExpense, AppError> {
        let amount: f64 = self
            .amount
            .parse()
            .map_err(|_| AppError::Validation("Invalid amount".into()))?;

        Ok(NewExpense {
            date: self.date.clone(),
            amount_cents: (amount * 100.0).round() as i64,
            currency: self.currency.clone(),
            description: self.description.clone(),
            category_id: self.category_id,
            notes: self.notes.clone(),
            tag_ids: self.tag_ids.clone(),
            // Extended fields are not editable via the simple form
            value_date: None,
            payer: None,
            payee: None,
            reference: None,
            transaction_type: None,
            counterparty_iban: None,
            creditor_id: None,
            mandate_reference: None,
            customer_reference: None,
        })
    }
}

pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<ExpenseFilterParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let settings_map = settings::get_all_settings(&conn)?;
    let app_settings = Settings::from_map(settings_map);

    let page = params.page.unwrap_or(1).max(1);
    let page_size = app_settings.page_size;

    let filter = expenses::ExpenseFilter {
        search: params.search.clone(),
        category_id: params.category_id,
        tag_id: params.tag_id,
        from_date: params.from_date.clone(),
        to_date: params.to_date.clone(),
        limit: Some(page_size),
        offset: Some((page - 1) * page_size),
    };

    let expense_list = expenses::list_expenses(&conn, &filter)?;
    let total_count = expenses::count_expenses(&conn, &filter)?;
    let cats = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

    let template = ExpensesTemplate {
        title: "Expenses".into(),
        settings: app_settings,
        manifest: state.manifest.clone(),
        expenses: expense_list,
        categories: cats,
        tags: tag_list,
        total_count,
        page,
        page_size,
        filter: params,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn table_partial(
    State(state): State<AppState>,
    Query(params): Query<ExpenseFilterParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let settings_map = settings::get_all_settings(&conn)?;
    let app_settings = Settings::from_map(settings_map);

    let page = params.page.unwrap_or(1).max(1);
    let page_size = app_settings.page_size;

    let filter = expenses::ExpenseFilter {
        search: params.search.clone(),
        category_id: params.category_id,
        tag_id: params.tag_id,
        from_date: params.from_date.clone(),
        to_date: params.to_date.clone(),
        limit: Some(page_size),
        offset: Some((page - 1) * page_size),
    };

    let expense_list = expenses::list_expenses(&conn, &filter)?;
    let total_count = expenses::count_expenses(&conn, &filter)?;

    let template = ExpenseTableTemplate {
        expenses: expense_list,
        total_count,
        page,
        page_size,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn show(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let expense = expenses::get_expense(&conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Expense {} not found", id)))?;

    let settings_map = settings::get_all_settings(&conn)?;
    let app_settings = Settings::from_map(settings_map);

    let cats = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

    let template = ExpenseDetailTemplate {
        title: format!("Transaction #{}", id),
        settings: app_settings,
        manifest: state.manifest.clone(),
        expense,
        categories: cats,
        tags: tag_list,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn new_form(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let cats = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

    let template = ExpenseFormTemplate {
        expense: None,
        categories: cats,
        tags: tag_list,
        is_edit: false,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn edit_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let expense = expenses::get_expense(&conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Expense {} not found", id)))?;

    let cats = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

    let template = ExpenseFormTemplate {
        expense: Some(expense),
        categories: cats,
        tags: tag_list,
        is_edit: true,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<ExpenseFormData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let new_expense = form.to_new_expense()?;
    let id = expenses::create_expense(&conn, &new_expense)?;

    let expense = expenses::get_expense(&conn, id)?
        .ok_or_else(|| AppError::Internal("Failed to retrieve created expense".into()))?;

    let template = ExpenseRowTemplate { expense };

    Ok(Html(template.render().unwrap()))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<ExpenseFormData>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let new_expense = form.to_new_expense()?;
    expenses::update_expense(&conn, id, &new_expense)?;

    let expense = expenses::get_expense(&conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Expense {} not found", id)))?;

    let template = ExpenseRowTemplate { expense };

    Ok(Html(template.render().unwrap()))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    expenses::delete_expense(&conn, id)?;

    Ok(Html(String::new()))
}

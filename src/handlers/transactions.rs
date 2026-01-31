use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Redirect};
use axum::{Form, Json};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::date_utils::{DateFilterable, DatePreset, DateRange};
use crate::db::queries::{accounts, categories, tags, transactions};
use crate::error::{AppError, AppResult, RenderHtml};
use crate::models::{
    Account, AccountType, CategoryWithPath, NewTransaction, Settings, Tag, TransactionWithRelations,
};
use crate::sort_utils::{Sortable, SortableColumn, TableSort};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

/// Sortable columns for the transactions table.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum TransactionSortColumn {
    #[default]
    Date,
    Description,
    Counterparty,
    Category,
    Amount,
}

impl SortableColumn for TransactionSortColumn {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "date" => Some(Self::Date),
            "description" => Some(Self::Description),
            "counterparty" => Some(Self::Counterparty),
            "category" => Some(Self::Category),
            "amount" => Some(Self::Amount),
            _ => None,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Description => "description",
            Self::Counterparty => "counterparty",
            Self::Category => "category",
            Self::Amount => "amount",
        }
    }

    fn sql_expression(&self) -> &'static str {
        match self {
            Self::Date => "e.date",
            Self::Description => "e.description",
            Self::Counterparty => "e.payee",
            Self::Category => "c.name",
            Self::Amount => "e.amount_cents",
        }
    }
}

#[derive(Template)]
#[template(path = "pages/transactions.html")]
pub struct TransactionsTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub transactions: Vec<TransactionWithRelations>,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
    pub total_count: i64,
    pub delete_count: i64,
    pub page: i64,
    pub page_size: i64,
    pub filter: TransactionFilterParams,
    pub date_range: DateRange,
    pub presets: &'static [DatePreset],
    pub sort: TableSort<TransactionSortColumn>,
}

#[derive(Template)]
#[template(path = "partials/transaction_table.html")]
pub struct TransactionTableTemplate {
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub transactions: Vec<TransactionWithRelations>,
    pub total_count: i64,
    pub page: i64,
    pub page_size: i64,
    pub filter: TransactionFilterParams,
    pub date_range: DateRange,
    pub sort: TableSort<TransactionSortColumn>,
}

#[derive(Template)]
#[template(path = "components/transaction_form.html")]
pub struct TransactionFormTemplate {
    pub icons: crate::filters::Icons,
    pub transaction: Option<TransactionWithRelations>,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
    pub is_edit: bool,
}

#[derive(Template)]
#[template(path = "pages/transaction_new.html")]
pub struct TransactionNewTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
    pub accounts: Vec<Account>,
}

#[derive(Template)]
#[template(path = "components/transaction_row.html")]
pub struct TransactionRowTemplate {
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub transaction: TransactionWithRelations,
}

#[derive(Template)]
#[template(path = "pages/transaction_detail.html")]
pub struct TransactionDetailTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub transaction: TransactionWithRelations,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
    pub accounts: Vec<Account>,
}

#[derive(Template)]
#[template(path = "pages/transaction_edit.html")]
pub struct TransactionEditTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub transaction: TransactionWithRelations,
    pub categories: Vec<CategoryWithPath>,
    pub tags: Vec<Tag>,
    pub accounts: Vec<Account>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct TransactionFilterParams {
    pub search: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::form_utils::deserialize_optional_i64"
    )]
    pub category_id: Option<i64>,
    #[serde(
        default,
        deserialize_with = "crate::form_utils::deserialize_optional_i64"
    )]
    pub tag_id: Option<i64>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub page: Option<i64>,
    pub preset: Option<String>,
    pub nav: Option<String>, // "prev" or "next"
    pub sort: Option<String>,
    pub dir: Option<String>,
}

impl DateFilterable for TransactionFilterParams {
    fn from_date(&self) -> Option<&String> {
        self.from_date.as_ref()
    }

    fn to_date(&self) -> Option<&String> {
        self.to_date.as_ref()
    }

    fn preset(&self) -> Option<&String> {
        self.preset.as_ref()
    }

    fn nav(&self) -> Option<&String> {
        self.nav.as_ref()
    }
}

impl Sortable for TransactionFilterParams {
    fn sort_by(&self) -> Option<&String> {
        self.sort.as_ref()
    }

    fn sort_dir(&self) -> Option<&String> {
        self.dir.as_ref()
    }
}

impl TransactionFilterParams {
    pub fn is_uncategorized(&self) -> bool {
        self.category_id == Some(0)
    }

    pub fn matches_category(&self, id: &i64) -> bool {
        self.category_id == Some(*id)
    }

    /// Returns filter query string (search, category_id, tag_id).
    pub fn base_query_string(&self) -> String {
        let mut parts = Vec::new();
        if let Some(search) = &self.search {
            if !search.is_empty() {
                parts.push(format!("search={}", urlencoding::encode(search)));
            }
        }
        if let Some(cat_id) = self.category_id {
            parts.push(format!("category_id={}", cat_id));
        }
        if let Some(tag_id) = self.tag_id {
            parts.push(format!("tag_id={}", tag_id));
        }
        parts.join("&")
    }

    /// Returns full query string including sort parameters.
    pub fn full_query_string(&self) -> String {
        let mut parts = Vec::new();
        let base = self.base_query_string();
        if !base.is_empty() {
            parts.push(base);
        }
        if let Some(sort) = &self.sort {
            parts.push(format!("sort={}", sort));
        }
        if let Some(dir) = &self.dir {
            parts.push(format!("dir={}", dir));
        }
        parts.join("&")
    }

    /// Returns query string combining date range and filter params (for preserving state in sort links).
    pub fn preserve_query_string(&self, date_range: &DateRange) -> String {
        let mut qs = date_range.query_string();
        let base = self.base_query_string();
        if !base.is_empty() {
            qs.push('&');
            qs.push_str(&base);
        }
        qs
    }
}

#[derive(Debug, Deserialize)]
pub struct TransactionFormData {
    pub date: String,
    pub amount: String,
    pub currency: String,
    pub description: String,
    #[serde(
        default,
        deserialize_with = "crate::form_utils::deserialize_optional_i64"
    )]
    pub category_id: Option<i64>,
    #[serde(
        default,
        deserialize_with = "crate::form_utils::deserialize_optional_i64"
    )]
    pub account_id: Option<i64>,
    pub notes: Option<String>,
    #[serde(default)]
    pub tag_ids: Vec<i64>,
}

impl TransactionFormData {
    fn to_new_transaction(&self) -> Result<NewTransaction, AppError> {
        let amount: f64 = self
            .amount
            .parse()
            .map_err(|_| AppError::Validation("Invalid amount".into()))?;

        Ok(NewTransaction {
            date: self.date.clone(),
            amount_cents: (amount * 100.0).round() as i64,
            currency: self.currency.clone(),
            description: self.description.clone(),
            category_id: self.category_id,
            account_id: self.account_id,
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
    Query(params): Query<TransactionFilterParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = state.load_settings()?;

    let page = params.page.unwrap_or(1).max(1);
    let page_size = app_settings.page_size;

    let date_range = params.resolve_date_range();
    let sort: TableSort<TransactionSortColumn> = params.resolve_sort();

    let filter = transactions::TransactionFilter {
        search: params.search.clone(),
        category_id: if params.is_uncategorized() {
            None
        } else {
            params.category_id
        },
        tag_id: params.tag_id,
        from_date: Some(date_range.from_str()),
        to_date: Some(date_range.to_str()),
        limit: Some(page_size),
        offset: Some((page - 1) * page_size),
        sort_sql: Some(sort.sql_order_by()),
        uncategorized_only: params.is_uncategorized(),
    };

    let transaction_list = transactions::list_transactions(&conn, &filter)?;
    let total_count = transactions::count_transactions(&conn, &filter)?;
    let cats = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;

    let template = TransactionsTemplate {
        title: "Transactions".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        transactions: transaction_list,
        categories: cats,
        tags: tag_list,
        total_count,
        delete_count: total_count,
        page,
        page_size,
        filter: params,
        date_range,
        presets: DatePreset::all(),
        sort,
    };

    template.render_html()
}

pub async fn table_partial(
    State(state): State<AppState>,
    Query(params): Query<TransactionFilterParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = state.load_settings()?;

    let page = params.page.unwrap_or(1).max(1);
    let page_size = app_settings.page_size;

    let date_range = params.resolve_date_range();
    let sort: TableSort<TransactionSortColumn> = params.resolve_sort();

    let filter = transactions::TransactionFilter {
        search: params.search.clone(),
        category_id: if params.is_uncategorized() {
            None
        } else {
            params.category_id
        },
        tag_id: params.tag_id,
        from_date: Some(date_range.from_str()),
        to_date: Some(date_range.to_str()),
        limit: Some(page_size),
        offset: Some((page - 1) * page_size),
        sort_sql: Some(sort.sql_order_by()),
        uncategorized_only: params.is_uncategorized(),
    };

    let transaction_list = transactions::list_transactions(&conn, &filter)?;
    let total_count = transactions::count_transactions(&conn, &filter)?;

    let template = TransactionTableTemplate {
        settings: app_settings,
        icons: crate::filters::Icons,
        transactions: transaction_list,
        total_count,
        page,
        page_size,
        filter: params,
        date_range,
        sort,
    };

    template.render_html()
}

pub async fn show(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let transaction = transactions::get_transaction(&conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Transaction {} not found", id)))?;

    let app_settings = state.load_settings()?;

    let cats = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;
    let cash_accounts = accounts::list_accounts_by_type(&conn, AccountType::Cash)?;

    let template = TransactionDetailTemplate {
        title: format!("Transaction #{}", id),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        transaction,
        categories: cats,
        tags: tag_list,
        accounts: cash_accounts,
    };

    template.render_html()
}

pub async fn new_form(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = state.load_settings()?;
    let cats = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;
    let cash_accounts = accounts::list_accounts_by_type(&conn, AccountType::Cash)?;

    let template = TransactionNewTemplate {
        title: "Add Transaction".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        categories: cats,
        tags: tag_list,
        accounts: cash_accounts,
    };

    template.render_html()
}

pub async fn edit_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let transaction = transactions::get_transaction(&conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Transaction {} not found", id)))?;

    let app_settings = state.load_settings()?;

    let cats = categories::list_categories_with_path(&conn)?;
    let tag_list = tags::list_tags(&conn)?;
    let cash_accounts = accounts::list_accounts_by_type(&conn, AccountType::Cash)?;

    let template = TransactionEditTemplate {
        title: "Edit Transaction".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        transaction,
        categories: cats,
        tags: tag_list,
        accounts: cash_accounts,
    };

    template.render_html()
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<TransactionFormData>,
) -> AppResult<Redirect> {
    debug!(description = %form.description, amount = %form.amount, "Creating transaction");
    let conn = state.db.get()?;

    let new_transaction = form.to_new_transaction()?;
    let id = transactions::create_transaction(&conn, &new_transaction)?;
    info!(transaction_id = id, "Transaction created via web form");

    Ok(Redirect::to("/transactions"))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<TransactionFormData>,
) -> AppResult<Redirect> {
    debug!(transaction_id = id, "Updating transaction");
    let conn = state.db.get()?;

    let new_transaction = form.to_new_transaction()?;
    transactions::update_transaction(&conn, id, &new_transaction)?;
    info!(transaction_id = id, "Transaction updated via web form");

    Ok(Redirect::to(&format!("/transactions/{}", id)))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    info!(transaction_id = id, "Deleting transaction");
    let conn = state.db.get()?;

    transactions::delete_transaction(&conn, id)?;

    Ok(Html(String::new()))
}

pub async fn delete_all(State(state): State<AppState>) -> AppResult<Html<String>> {
    warn!("Deleting all transactions");
    let conn = state.db.get()?;

    transactions::delete_all_transactions(&conn)?;

    Ok(Html(String::new()))
}

#[derive(Serialize)]
struct TransactionExport {
    date: String,
    amount_cents: i64,
    currency: String,
    description: String,
    category_name: Option<String>,
    account_name: Option<String>,
    notes: Option<String>,
    tags: Vec<String>,
    value_date: Option<String>,
    payer: Option<String>,
    payee: Option<String>,
    reference: Option<String>,
    transaction_type: Option<String>,
    counterparty_iban: Option<String>,
    creditor_id: Option<String>,
    mandate_reference: Option<String>,
    customer_reference: Option<String>,
}

pub async fn export(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let conn = state.db.get()?;

    let filter = crate::db::queries::transactions::TransactionFilter::default();

    let txns = transactions::list_transactions(&conn, &filter)?;

    let export_data: Vec<TransactionExport> = txns
        .iter()
        .map(|t| TransactionExport {
            date: t.transaction.date.clone(),
            amount_cents: t.transaction.amount_cents,
            currency: t.transaction.currency.clone(),
            description: t.transaction.description.clone(),
            category_name: t.category_name.clone(),
            account_name: t.account_name.clone(),
            notes: t.transaction.notes.clone(),
            tags: t.tags.iter().map(|tag| tag.name.clone()).collect(),
            value_date: t.transaction.value_date.clone(),
            payer: t.transaction.payer.clone(),
            payee: t.transaction.payee.clone(),
            reference: t.transaction.reference.clone(),
            transaction_type: t.transaction.transaction_type.clone(),
            counterparty_iban: t.transaction.counterparty_iban.clone(),
            creditor_id: t.transaction.creditor_id.clone(),
            mandate_reference: t.transaction.mandate_reference.clone(),
            customer_reference: t.transaction.customer_reference.clone(),
        })
        .collect();

    let json = serde_json::to_string_pretty(&export_data)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {}", e)))?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"transactions.json\"",
            ),
        ],
        json,
    ))
}

#[derive(Deserialize)]
struct TransactionImport {
    date: String,
    amount_cents: i64,
    #[serde(default = "default_currency")]
    currency: String,
    description: String,
    category_name: Option<String>,
    account_name: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    value_date: Option<String>,
    #[serde(default)]
    payer: Option<String>,
    #[serde(default)]
    payee: Option<String>,
    #[serde(default)]
    reference: Option<String>,
    #[serde(default)]
    transaction_type: Option<String>,
    #[serde(default)]
    counterparty_iban: Option<String>,
    #[serde(default)]
    creditor_id: Option<String>,
    #[serde(default)]
    mandate_reference: Option<String>,
    #[serde(default)]
    customer_reference: Option<String>,
}

fn default_currency() -> String {
    "USD".to_string()
}

pub async fn import(
    State(state): State<AppState>,
    Json(value): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let data: Vec<TransactionImport> = serde_json::from_value(value)
        .map_err(|e| AppError::Validation(format!("Invalid JSON format: {}", e)))?;

    let conn = state.db.get()?;

    // Build lookup maps for category and account names
    let cat_list = categories::list_categories(&conn)?;
    let cat_name_to_id: std::collections::HashMap<String, i64> =
        cat_list.iter().map(|c| (c.name.clone(), c.id)).collect();

    let account_list = accounts::list_accounts(&conn)?;
    let account_name_to_id: std::collections::HashMap<String, i64> = account_list
        .iter()
        .map(|a| (a.name.clone(), a.id))
        .collect();

    let tag_list = tags::list_tags(&conn)?;
    let tag_name_to_id: std::collections::HashMap<String, i64> =
        tag_list.iter().map(|t| (t.name.clone(), t.id)).collect();

    let mut created = 0;
    for item in data {
        let category_id = item
            .category_name
            .as_ref()
            .and_then(|name| cat_name_to_id.get(name).copied());

        let account_id = item
            .account_name
            .as_ref()
            .and_then(|name| account_name_to_id.get(name).copied());

        let tag_ids: Vec<i64> = item
            .tags
            .iter()
            .filter_map(|name| tag_name_to_id.get(name).copied())
            .collect();

        let new_txn = NewTransaction {
            date: item.date,
            amount_cents: item.amount_cents,
            currency: item.currency,
            description: item.description,
            category_id,
            account_id,
            notes: item.notes,
            tag_ids,
            value_date: item.value_date,
            payer: item.payer,
            payee: item.payee,
            reference: item.reference,
            transaction_type: item.transaction_type,
            counterparty_iban: item.counterparty_iban,
            creditor_id: item.creditor_id,
            mandate_reference: item.mandate_reference,
            customer_reference: item.customer_reference,
        };

        transactions::create_transaction(&conn, &new_txn)?;
        created += 1;
    }

    Ok(Json(serde_json::json!({
        "imported": created,
        "message": format!("Successfully imported {} transactions", created)
    })))
}

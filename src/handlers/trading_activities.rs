use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Redirect};
use axum::{Form, Json};
use serde::{Deserialize, Serialize};

use crate::date_utils::{DateFilterable, DatePreset, DateRange};
use crate::db::queries::{accounts, settings, trading};
use crate::error::{AppError, AppResult, RenderHtml};
use crate::models::{NewTradingActivity, Settings, TradingActivity, TradingActivityType};
use crate::sort_utils::{Sortable, SortableColumn, TableSort};
use crate::state::{AppState, JsManifest};
use crate::VERSION;

/// Sortable columns for the trading activities table.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum ActivitySortColumn {
    #[default]
    Date,
    Symbol,
    Type,
    Quantity,
    Price,
    Total,
    Fee,
}

impl SortableColumn for ActivitySortColumn {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "date" => Some(Self::Date),
            "symbol" => Some(Self::Symbol),
            "type" => Some(Self::Type),
            "quantity" => Some(Self::Quantity),
            "price" => Some(Self::Price),
            "total" => Some(Self::Total),
            "fee" => Some(Self::Fee),
            _ => None,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Symbol => "symbol",
            Self::Type => "type",
            Self::Quantity => "quantity",
            Self::Price => "price",
            Self::Total => "total",
            Self::Fee => "fee",
        }
    }

    fn sql_expression(&self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Symbol => "symbol",
            Self::Type => "activity_type",
            Self::Quantity => "quantity",
            Self::Price => "unit_price_cents",
            Self::Total => "(COALESCE(quantity, 1) * COALESCE(unit_price_cents, 0))",
            Self::Fee => "fee_cents",
        }
    }
}

#[derive(Template)]
#[template(path = "pages/trading_activities.html")]
pub struct TradingActivitiesTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub activities: Vec<TradingActivity>,
    pub symbols: Vec<String>,
    pub activity_types: &'static [TradingActivityType],
    pub total_count: i64,
    pub page: i64,
    pub page_size: i64,
    pub filter: TradingActivityFilterParams,
    pub date_range: DateRange,
    pub presets: &'static [DatePreset],
    pub sort: TableSort<ActivitySortColumn>,
}

#[derive(Template)]
#[template(path = "partials/trading_activity_table.html")]
pub struct TradingActivityTableTemplate {
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub activities: Vec<TradingActivity>,
    pub total_count: i64,
    pub page: i64,
    pub page_size: i64,
    pub filter: TradingActivityFilterParams,
    pub date_range: DateRange,
    pub sort: TableSort<ActivitySortColumn>,
}

#[derive(Template)]
#[template(path = "components/trading_activity_form.html")]
pub struct TradingActivityFormTemplate {
    pub activity: Option<TradingActivity>,
    pub symbols: Vec<String>,
    pub activity_types: &'static [TradingActivityType],
    pub is_edit: bool,
}

#[derive(Template)]
#[template(path = "pages/trading_activity_new.html")]
pub struct TradingActivityNewTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub symbols: Vec<String>,
    pub activity_types: &'static [TradingActivityType],
}

#[derive(Template)]
#[template(path = "components/trading_activity_row.html")]
pub struct TradingActivityRowTemplate {
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub activity: TradingActivity,
}

#[derive(Template)]
#[template(path = "pages/trading_activity_detail.html")]
pub struct TradingActivityDetailTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub activity: TradingActivity,
}

#[derive(Template)]
#[template(path = "pages/trading_activity_edit.html")]
pub struct TradingActivityEditTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub activity: TradingActivity,
    pub symbols: Vec<String>,
    pub activity_types: &'static [TradingActivityType],
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct TradingActivityFilterParams {
    pub symbol: Option<String>,
    pub activity_type: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub page: Option<i64>,
    pub preset: Option<String>,
    pub sort: Option<String>,
    pub dir: Option<String>,
}

impl DateFilterable for TradingActivityFilterParams {
    fn from_date(&self) -> Option<&String> {
        self.from_date.as_ref()
    }

    fn to_date(&self) -> Option<&String> {
        self.to_date.as_ref()
    }

    fn preset(&self) -> Option<&String> {
        self.preset.as_ref()
    }
}

impl Sortable for TradingActivityFilterParams {
    fn sort_by(&self) -> Option<&String> {
        self.sort.as_ref()
    }

    fn sort_dir(&self) -> Option<&String> {
        self.dir.as_ref()
    }
}

impl TradingActivityFilterParams {
    pub fn matches_symbol(&self, sym: &str) -> bool {
        self.symbol.as_deref() == Some(sym)
    }

    pub fn matches_activity_type(&self, at: &TradingActivityType) -> bool {
        self.activity_type.as_deref() == Some(at.as_str())
    }

    /// Returns filter query string (symbol, activity_type).
    pub fn base_query_string(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref symbol) = self.symbol {
            if !symbol.is_empty() {
                parts.push(format!("symbol={}", urlencoding::encode(symbol)));
            }
        }
        if let Some(ref activity_type) = self.activity_type {
            if !activity_type.is_empty() {
                parts.push(format!("activity_type={}", activity_type));
            }
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
pub struct TradingActivityFormData {
    pub date: String,
    pub symbol: String,
    pub quantity: Option<String>,
    pub activity_type: String,
    pub unit_price: Option<String>,
    pub currency: String,
    pub fee: Option<String>,
    pub account_id: Option<i64>,
    pub notes: Option<String>,
}

impl TradingActivityFormData {
    fn to_new_activity(&self) -> Result<NewTradingActivity, AppError> {
        let activity_type: TradingActivityType = self
            .activity_type
            .parse()
            .map_err(|_| AppError::Validation("Invalid activity type".into()))?;

        let quantity = self
            .quantity
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<f64>()
                    .map_err(|_| AppError::Validation("Invalid quantity".into()))
            })
            .transpose()?;

        let unit_price_cents = self
            .unit_price
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<f64>()
                    .map(|v| (v * 100.0).round() as i64)
                    .map_err(|_| AppError::Validation("Invalid unit price".into()))
            })
            .transpose()?;

        let fee_cents = self
            .fee
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<f64>()
                    .map(|v| (v * 100.0).round() as i64)
                    .map_err(|_| AppError::Validation("Invalid fee".into()))
            })
            .transpose()?
            .unwrap_or(0);

        Ok(NewTradingActivity {
            date: self.date.clone(),
            symbol: self.symbol.clone(),
            quantity,
            activity_type,
            unit_price_cents,
            currency: self.currency.clone(),
            fee_cents,
            account_id: self.account_id,
            notes: self.notes.clone().filter(|s| !s.is_empty()),
        })
    }
}

pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<TradingActivityFilterParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

    let page = params.page.unwrap_or(1).max(1);
    let page_size = app_settings.page_size;

    let date_range = params.resolve_date_range();
    let sort: TableSort<ActivitySortColumn> = params.resolve_sort();

    let activity_type = params
        .activity_type
        .as_ref()
        .and_then(|s| s.parse::<TradingActivityType>().ok());

    let filter = trading::TradingActivityFilter {
        symbol: params.symbol.clone().filter(|s| !s.is_empty()),
        activity_type,
        from_date: Some(date_range.from_str()),
        to_date: Some(date_range.to_str()),
        limit: Some(page_size),
        offset: Some((page - 1) * page_size),
        sort_sql: Some(sort.sql_order_by()),
    };

    let activity_list = trading::list_activities(&conn, &filter)?;
    let total_count = trading::count_activities(&conn, &filter)?;
    let symbols = trading::get_unique_symbols(&conn)?;

    let template = TradingActivitiesTemplate {
        title: "Trading Activities".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        activities: activity_list,
        symbols,
        activity_types: TradingActivityType::all(),
        total_count,
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
    Query(params): Query<TradingActivityFilterParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;

    let page = params.page.unwrap_or(1).max(1);
    let page_size = app_settings.page_size;

    let date_range = params.resolve_date_range();
    let sort: TableSort<ActivitySortColumn> = params.resolve_sort();

    let activity_type = params
        .activity_type
        .as_ref()
        .and_then(|s| s.parse::<TradingActivityType>().ok());

    let filter = trading::TradingActivityFilter {
        symbol: params.symbol.clone().filter(|s| !s.is_empty()),
        activity_type,
        from_date: Some(date_range.from_str()),
        to_date: Some(date_range.to_str()),
        limit: Some(page_size),
        offset: Some((page - 1) * page_size),
        sort_sql: Some(sort.sql_order_by()),
    };

    let activity_list = trading::list_activities(&conn, &filter)?;
    let total_count = trading::count_activities(&conn, &filter)?;

    let template = TradingActivityTableTemplate {
        settings: app_settings,
        icons: crate::filters::Icons,
        activities: activity_list,
        total_count,
        page,
        page_size,
        filter: params,
        date_range,
        sort,
    };

    template.render_html()
}

pub async fn detail(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let activity = trading::get_activity(&conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Activity {} not found", id)))?;

    let app_settings = settings::get_settings(&conn)?;

    let template = TradingActivityDetailTemplate {
        title: format!("{} - {}", activity.symbol, activity.activity_type.label()),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        activity,
    };

    template.render_html()
}

pub async fn new_form(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let app_settings = settings::get_settings(&conn)?;
    let symbols = trading::get_unique_symbols(&conn)?;

    let template = TradingActivityNewTemplate {
        title: "Add Activity".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        symbols,
        activity_types: TradingActivityType::all(),
    };

    template.render_html()
}

pub async fn edit_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    let activity = trading::get_activity(&conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Activity {} not found", id)))?;

    let app_settings = settings::get_settings(&conn)?;

    let symbols = trading::get_unique_symbols(&conn)?;

    let template = TradingActivityEditTemplate {
        title: "Edit Activity".into(),
        settings: app_settings,
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        activity,
        symbols,
        activity_types: TradingActivityType::all(),
    };

    template.render_html()
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<TradingActivityFormData>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    let new_activity = form.to_new_activity()?;
    trading::create_activity(&conn, &new_activity)?;

    Ok(Redirect::to("/trading/activities"))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<TradingActivityFormData>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    let new_activity = form.to_new_activity()?;
    trading::update_activity(&conn, id, &new_activity)?;

    Ok(Redirect::to("/trading/activities"))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    trading::delete_activity(&conn, id)?;

    Ok(Html(String::new()))
}

pub async fn delete_all(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;

    trading::delete_all_activities(&conn)?;

    Ok(Html(String::new()))
}

#[derive(Serialize)]
struct TradingActivityExport {
    date: String,
    symbol: String,
    quantity: Option<f64>,
    activity_type: TradingActivityType,
    unit_price_cents: Option<i64>,
    currency: String,
    fee_cents: i64,
    account_name: Option<String>,
    notes: Option<String>,
}

pub async fn export(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let conn = state.db.get()?;

    let filter = crate::db::queries::trading::TradingActivityFilter {
        symbol: None,
        activity_type: None,
        from_date: None,
        to_date: None,
        limit: None,
        offset: None,
        sort_sql: None,
    };

    let activities = trading::list_activities(&conn, &filter)?;

    // Build account id -> name map for export
    let account_list = accounts::list_accounts(&conn)?;
    let account_id_to_name: std::collections::HashMap<i64, String> = account_list
        .iter()
        .map(|a| (a.id, a.name.clone()))
        .collect();

    let export_data: Vec<TradingActivityExport> = activities
        .iter()
        .map(|a| TradingActivityExport {
            date: a.date.clone(),
            symbol: a.symbol.clone(),
            quantity: a.quantity,
            activity_type: a.activity_type,
            unit_price_cents: a.unit_price_cents,
            currency: a.currency.clone(),
            fee_cents: a.fee_cents,
            account_name: a
                .account_id
                .and_then(|id| account_id_to_name.get(&id).cloned()),
            notes: a.notes.clone(),
        })
        .collect();

    let json = serde_json::to_string_pretty(&export_data)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {}", e)))?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"trading_activities.json\"",
            ),
        ],
        json,
    ))
}

#[derive(Deserialize)]
struct TradingActivityImport {
    date: String,
    symbol: String,
    quantity: Option<f64>,
    activity_type: TradingActivityType,
    unit_price_cents: Option<i64>,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default)]
    fee_cents: i64,
    account_name: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

fn default_currency() -> String {
    "USD".to_string()
}

pub async fn import(
    State(state): State<AppState>,
    Json(value): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let data: Vec<TradingActivityImport> = serde_json::from_value(value)
        .map_err(|e| AppError::Validation(format!("Invalid JSON format: {}", e)))?;

    let conn = state.db.get()?;

    let account_list = accounts::list_accounts(&conn)?;
    let account_name_to_id: std::collections::HashMap<String, i64> = account_list
        .iter()
        .map(|a| (a.name.clone(), a.id))
        .collect();

    let mut created = 0;
    for item in data {
        let account_id = item
            .account_name
            .as_ref()
            .and_then(|name| account_name_to_id.get(name).copied());

        let new_activity = NewTradingActivity {
            date: item.date,
            symbol: item.symbol,
            quantity: item.quantity,
            activity_type: item.activity_type,
            unit_price_cents: item.unit_price_cents,
            currency: item.currency,
            fee_cents: item.fee_cents,
            account_id,
            notes: item.notes,
        };

        trading::create_activity(&conn, &new_activity)?;
        created += 1;
    }

    Ok(Json(serde_json::json!({
        "imported": created,
        "message": format!("Successfully imported {} trading activities", created)
    })))
}

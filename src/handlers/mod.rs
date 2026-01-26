pub mod accounts;
pub mod api;
pub mod api_logs;
pub mod balances;
pub mod categories;
pub mod dashboard;
pub mod import;
pub mod market_data;
pub mod net_worth;
pub mod rules;
pub mod settings;
pub mod spending;
pub mod tags;
pub mod trading_activities;
pub mod trading_import;
pub mod trading_positions;
pub mod transactions;

use axum::routing::{delete, get, post, put};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        // Pages
        .route("/", get(dashboard::index))
        .route("/balances", get(balances::index))
        .route("/spending", get(spending::index))
        .route("/transactions", get(transactions::index))
        .route("/import", get(import::index))
        .route("/settings", get(settings::index))
        // Transaction CRUD
        .route("/transactions/new", get(transactions::new_form))
        .route("/transactions/create", post(transactions::create))
        .route("/transactions/table", get(transactions::table_partial))
        .route("/transactions/:id", get(transactions::show))
        .route("/transactions/:id/edit", get(transactions::edit_form))
        .route("/transactions/:id/update", post(transactions::update))
        .route("/transactions/:id/delete", delete(transactions::delete))
        .route("/transactions/delete-all", delete(transactions::delete_all))
        .route("/transactions/export", get(transactions::export))
        .route("/transactions/import", post(transactions::import))
        // Category management
        .route("/categories", get(categories::index))
        .route("/categories/new", get(categories::new_form))
        .route("/categories/create", post(categories::create))
        .route("/categories/export", get(categories::export))
        .route("/categories/import", post(categories::import))
        .route("/categories/delete-all", delete(categories::delete_all))
        .route("/categories/:id", put(categories::update))
        .route("/categories/:id", delete(categories::delete))
        // Account management
        .route("/accounts", get(accounts::index))
        .route("/accounts/new", get(accounts::new_form))
        .route("/accounts/create", post(accounts::create))
        .route("/accounts/export", get(accounts::export))
        .route("/accounts/import", post(accounts::import))
        .route("/accounts/:id/edit", get(accounts::edit_form))
        .route("/accounts/:id/update", post(accounts::update))
        .route("/accounts/:id", delete(accounts::delete))
        .route("/accounts/delete-all", delete(accounts::delete_all))
        // Tag management
        .route("/tags", get(tags::index))
        .route("/tags/new", get(tags::new_form))
        .route("/tags/create", post(tags::create))
        .route("/tags/export", get(tags::export))
        .route("/tags/import", post(tags::import))
        .route("/tags/search", get(tags::search))
        .route("/tags/:id", delete(tags::delete))
        .route("/tags/delete-all", delete(tags::delete_all))
        // Rule management
        .route("/rules", get(rules::index))
        .route("/rules/new", get(rules::new_form))
        .route("/rules/create", post(rules::create))
        .route("/rules/export", get(rules::export))
        .route("/rules/import", post(rules::import))
        .route("/rules/:id", put(rules::update))
        .route("/rules/:id", delete(rules::delete))
        .route("/rules/delete-all", delete(rules::delete_all))
        // Import
        .route("/import/format", get(import::format))
        .route("/import/upload", post(import::upload))
        .route("/import/:session_id", get(import::wizard))
        .route("/import/:session_id/status", get(import::status))
        .route("/import/:session_id/status.json", get(import::status_json))
        .route("/import/:session_id/rows", get(import::rows))
        .route(
            "/import/:session_id/rows/:row_id/category",
            post(import::update_row_category),
        )
        .route(
            "/import/:session_id/categories",
            post(import::update_all_categories),
        )
        .route("/import/:session_id/confirm", post(import::confirm))
        .route("/import/:session_id/result", get(import::result))
        .route("/import/:session_id/cancel", get(import::cancel))
        // Trading Activities
        .route("/trading/activities", get(trading_activities::index))
        .route("/trading/activities/new", get(trading_activities::new_form))
        .route(
            "/trading/activities/create",
            post(trading_activities::create),
        )
        .route(
            "/trading/activities/table",
            get(trading_activities::table_partial),
        )
        .route("/trading/activities/:id", get(trading_activities::detail))
        .route(
            "/trading/activities/:id/edit",
            get(trading_activities::edit_form),
        )
        .route(
            "/trading/activities/:id/update",
            post(trading_activities::update),
        )
        .route(
            "/trading/activities/:id/delete",
            delete(trading_activities::delete),
        )
        .route(
            "/trading/activities/delete-all",
            delete(trading_activities::delete_all),
        )
        .route(
            "/trading/activities/export",
            get(trading_activities::export),
        )
        .route(
            "/trading/activities/import",
            post(trading_activities::import),
        )
        // Trading Positions
        .route("/trading/positions", get(trading_positions::index))
        .route(
            "/trading/positions/closed",
            get(trading_positions::closed_positions),
        )
        .route("/trading/positions/:symbol", get(trading_positions::detail))
        .route(
            "/api/positions/:symbol/chart",
            get(trading_positions::position_chart_data),
        )
        // Net Worth
        .route("/trading/net-worth", get(net_worth::index))
        .route("/api/net-worth/chart", get(net_worth::chart_data))
        .route(
            "/api/net-worth/top-transactions",
            get(net_worth::top_transactions),
        )
        // Trading Market Data
        .route("/trading/market-data", get(market_data::index))
        .route("/trading/market-data/refresh", post(market_data::refresh))
        .route(
            "/trading/market-data/refresh/:symbol",
            post(market_data::refresh_symbol),
        )
        .route("/trading/market-data/status", get(market_data::status))
        .route(
            "/trading/market-data/delete-all",
            delete(market_data::delete_all),
        )
        .route(
            "/trading/market-data/:symbol",
            get(market_data::symbol_detail),
        )
        .route(
            "/trading/market-data/:symbol/delete",
            delete(market_data::delete_symbol),
        )
        .route(
            "/api/market-data/:symbol",
            get(market_data::symbol_chart_data),
        )
        // API Logs
        .route("/trading/api-logs", get(api_logs::index))
        .route("/trading/api-logs/:id", get(api_logs::detail))
        .route("/api/api-logs/poll", get(api_logs::poll_errors))
        // Trading Import
        .route("/trading/import", get(trading_import::index))
        .route("/trading/import/format", get(trading_import::format))
        .route("/trading/import/upload", post(trading_import::upload))
        .route("/trading/import/:session_id", get(trading_import::wizard))
        .route(
            "/trading/import/:session_id/status",
            get(trading_import::status),
        )
        .route(
            "/trading/import/:session_id/status.json",
            get(trading_import::status_json),
        )
        .route(
            "/trading/import/:session_id/rows",
            get(trading_import::rows),
        )
        .route(
            "/trading/import/:session_id/confirm",
            post(trading_import::confirm),
        )
        .route(
            "/trading/import/:session_id/result",
            get(trading_import::result),
        )
        .route(
            "/trading/import/:session_id/cancel",
            get(trading_import::cancel),
        )
        // Settings
        .route("/settings/update", post(settings::update))
        .route("/settings/theme", post(settings::toggle_theme))
        .route("/settings/export-database", get(settings::export_database))
        .route("/settings/import-database", post(settings::import_database))
        .route("/settings/clear-database", delete(settings::clear_database))
        // API (JSON for charts)
        .route(
            "/api/analytics/spending-by-category",
            get(api::spending_by_category),
        )
        .route(
            "/api/analytics/spending-over-time",
            get(api::spending_over_time),
        )
        .route("/api/analytics/monthly-summary", get(api::monthly_summary))
        // Health check
        .route("/health", get(health))
}

async fn health() -> &'static str {
    "OK"
}

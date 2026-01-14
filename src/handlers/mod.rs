pub mod analytics;
pub mod api;
pub mod categories;
pub mod dashboard;
pub mod expenses;
pub mod import;
pub mod rules;
pub mod settings;
pub mod tags;

use axum::routing::{delete, get, post, put};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        // Pages
        .route("/", get(dashboard::index))
        .route("/analytics", get(analytics::index))
        .route("/expenses", get(expenses::index))
        .route("/import", get(import::index))
        .route("/settings", get(settings::index))
        // Expense CRUD
        .route("/expenses/new", get(expenses::new_form))
        .route("/expenses/create", post(expenses::create))
        .route("/expenses/table", get(expenses::table_partial))
        .route("/expenses/:id", get(expenses::show))
        .route("/expenses/:id/edit", get(expenses::edit_form))
        .route("/expenses/:id/update", post(expenses::update))
        .route("/expenses/:id/delete", delete(expenses::delete))
        // Category management
        .route("/categories", get(categories::index))
        .route("/categories/create", post(categories::create))
        .route("/categories/export", get(categories::export))
        .route("/categories/import", post(categories::import))
        .route("/categories/:id", put(categories::update))
        .route("/categories/:id", delete(categories::delete))
        // Tag management
        .route("/tags", get(tags::index))
        .route("/tags/create", post(tags::create))
        .route("/tags/export", get(tags::export))
        .route("/tags/import", post(tags::import))
        .route("/tags/search", get(tags::search))
        .route("/tags/:id", delete(tags::delete))
        // Rule management
        .route("/rules", get(rules::index))
        .route("/rules/create", post(rules::create))
        .route("/rules/export", get(rules::export))
        .route("/rules/import", post(rules::import))
        .route("/rules/:id", put(rules::update))
        .route("/rules/:id", delete(rules::delete))
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
        // Settings
        .route("/settings/update", post(settings::update))
        .route("/settings/theme", post(settings::toggle_theme))
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

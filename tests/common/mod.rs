//! Shared test utilities for integration tests.
//!
//! This module provides a `TestClient` that can be used to test the application
//! by making HTTP requests against an in-memory database. Methods are intentionally
//! broad to support various test scenarios across different test files.

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use http_body_util::BodyExt;
use solvency::auth;
use solvency::config::{AuthMode, Config};
use solvency::db::{create_in_memory_pool, migrations};
use solvency::handlers;
use solvency::state::{AppState, JsManifest, MarketDataRefreshState};
use solvency::xsrf::XsrfToken;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use tower_cookies::CookieManagerLayer;

/// A test client that simulates a browser session, allowing sequential requests
/// against the application.
pub struct TestClient {
    state: AppState,
}

impl TestClient {
    /// Create a new test client with a fresh in-memory database (unauthenticated mode).
    pub fn new() -> Self {
        Self::with_auth_mode(AuthMode::Unauthenticated)
    }

    /// Create a new test client with a specific authentication mode.
    pub fn with_auth_mode(auth_mode: AuthMode) -> Self {
        let pool = create_in_memory_pool().expect("Failed to create in-memory pool");
        {
            let conn = pool.get().expect("Failed to get connection");
            migrations::run_migrations(&conn, Path::new("migrations"))
                .expect("Failed to run migrations");
        }

        let config = Config {
            host: "127.0.0.1".into(),
            port: 7070,
            database_path: PathBuf::from(":memory:"),
            migrations_path: PathBuf::from("migrations"),
            static_path: PathBuf::from("static"),
            auth_mode,
        };

        let state = AppState {
            db: pool,
            config: Arc::new(config),
            manifest: JsManifest::default(),
            xsrf_token: XsrfToken::generate(),
            market_data_refresh: Arc::new(Mutex::new(MarketDataRefreshState::default())),
        };

        Self { state }
    }

    /// Get the router for making requests (without auth middleware for direct handler testing).
    pub fn router(&self) -> Router {
        handlers::routes().with_state(self.state.clone())
    }

    /// Get the full router with auth middleware applied (mimics production setup).
    pub fn router_with_auth(&self) -> Router {
        use axum::middleware;

        handlers::routes()
            .route("/login", get(auth::login_page))
            .route("/login", post(auth::login_submit))
            .route("/logout", post(auth::logout))
            .layer(middleware::from_fn_with_state(
                self.state.clone(),
                auth::auth_middleware,
            ))
            .layer(CookieManagerLayer::new())
            .with_state(self.state.clone())
    }

    /// Make a GET request and return status and body.
    pub async fn get(&self, uri: &str) -> (StatusCode, String) {
        let response = self
            .router()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();

        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&body).to_string())
    }

    /// Make a GET request with auth middleware and return status and body.
    pub async fn get_with_auth(&self, uri: &str) -> (StatusCode, String) {
        let response = self
            .router_with_auth()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();

        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&body).to_string())
    }

    /// Make a POST request with form data and return status and body.
    pub async fn post_form(&self, uri: &str, form_data: &[(&str, &str)]) -> (StatusCode, String) {
        let body = form_data
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let response = self
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        let status = response.status();
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&body_bytes).to_string())
    }

    /// Get JSON from an endpoint and parse it.
    pub async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        uri: &str,
    ) -> (StatusCode, Option<T>) {
        let (status, body) = self.get(uri).await;
        let parsed = serde_json::from_str(&body).ok();
        (status, parsed)
    }

    // =========================================================================
    // Helper methods for creating entities through the API
    // =========================================================================

    /// Create an account via POST and return success status.
    pub async fn create_account(&self, name: &str, account_type: &str) -> bool {
        let (status, _) = self
            .post_form(
                "/accounts/create",
                &[("name", name), ("account_type", account_type)],
            )
            .await;
        // Redirect (303) indicates success
        status == StatusCode::SEE_OTHER
    }

    /// Create a transaction via POST and return success status.
    pub async fn create_transaction(
        &self,
        date: &str,
        amount: &str,
        description: &str,
        account_id: Option<i64>,
        category_id: Option<i64>,
    ) -> bool {
        let mut form_data = vec![
            ("date", date.to_string()),
            ("amount", amount.to_string()),
            ("currency", "USD".to_string()),
            ("description", description.to_string()),
        ];

        if let Some(id) = account_id {
            form_data.push(("account_id", id.to_string()));
        }
        if let Some(id) = category_id {
            form_data.push(("category_id", id.to_string()));
        }

        let form_refs: Vec<(&str, &str)> =
            form_data.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let (status, _) = self.post_form("/transactions/create", &form_refs).await;
        status == StatusCode::SEE_OTHER
    }

    /// Create a trading activity via POST and return success status.
    pub async fn create_trading_activity(
        &self,
        date: &str,
        symbol: &str,
        activity_type: &str,
        quantity: &str,
        unit_price: &str,
    ) -> bool {
        let (status, _) = self
            .post_form(
                "/trading/activities/create",
                &[
                    ("date", date),
                    ("symbol", symbol),
                    ("activity_type", activity_type),
                    ("quantity", quantity),
                    ("unit_price", unit_price),
                    ("currency", "USD"),
                    ("fee", "0"),
                ],
            )
            .await;
        status == StatusCode::SEE_OTHER
    }
}

impl Default for TestClient {
    fn default() -> Self {
        Self::new()
    }
}

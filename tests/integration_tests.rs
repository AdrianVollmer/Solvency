//! Integration tests for Solvency API endpoints.
//!
//! These tests use an in-memory SQLite database and interact with the application
//! through its HTTP endpoints, simulating real browser sessions.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use solvency::config::{AuthMode, Config};
use solvency::db::{create_in_memory_pool, migrations};
use solvency::handlers;
use solvency::state::{AppState, JsManifest, MarketDataRefreshState};
use solvency::xsrf::XsrfToken;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

// =============================================================================
// Test Client - Simulates a browser session
// =============================================================================

/// A test client that simulates a browser session, allowing sequential requests
/// against the application.
struct TestClient {
    state: AppState,
}

impl TestClient {
    /// Create a new test client with a fresh in-memory database.
    fn new() -> Self {
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
            auth_mode: AuthMode::Unauthenticated,
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

    /// Get the router for making requests.
    fn router(&self) -> Router {
        handlers::routes().with_state(self.state.clone())
    }

    /// Make a GET request and return status and body.
    async fn get(&self, uri: &str) -> (StatusCode, String) {
        let response = self
            .router()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();

        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&body).to_string())
    }

    /// Make a POST request with form data and return status and body.
    async fn post_form(&self, uri: &str, form_data: &[(&str, &str)]) -> (StatusCode, String) {
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
    async fn get_json<T: serde::de::DeserializeOwned>(&self, uri: &str) -> (StatusCode, Option<T>) {
        let (status, body) = self.get(uri).await;
        let parsed = serde_json::from_str(&body).ok();
        (status, parsed)
    }

    // =========================================================================
    // Helper methods for creating entities through the API
    // =========================================================================

    /// Create an account via POST and return success status.
    async fn create_account(&self, name: &str, account_type: &str) -> bool {
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
    async fn create_transaction(
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
    async fn create_trading_activity(
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

// =============================================================================
// Balance Tests
// =============================================================================

mod balances {
    use super::*;

    /// Test that balances page loads with empty database.
    #[tokio::test]
    async fn test_balances_empty_db() {
        let client = TestClient::new();
        let (status, body) = client.get("/balances").await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("Balances"));
    }

    /// Test cash account balance is correctly computed by the application.
    #[tokio::test]
    async fn test_cash_account_balance_computation() {
        let client = TestClient::new();

        // 1. Create a cash account via API
        assert!(client.create_account("Checking", "Cash").await);

        // 2. Add transactions via API: +$100.00, -$30.00, +$50.00 = $120.00
        // Note: account_id=1 because it's the first account created
        assert!(
            client
                .create_transaction("2024-01-01", "100.00", "Deposit", Some(1), None)
                .await
        );
        assert!(
            client
                .create_transaction("2024-01-02", "-30.00", "Groceries", Some(1), Some(1))
                .await
        );
        assert!(
            client
                .create_transaction("2024-01-03", "50.00", "Refund", Some(1), None)
                .await
        );

        // 3. Get balances page and verify the computed value
        let (status, body) = client.get("/balances").await;

        assert_eq!(status, StatusCode::OK);
        // The page should show $120.00 for this account
        assert!(
            body.contains("$120.00") || body.contains("120.00"),
            "Expected balance of $120.00 not found in response"
        );
    }

    /// Test negative balance (overdraft) is correctly computed.
    #[tokio::test]
    async fn test_negative_balance() {
        let client = TestClient::new();

        assert!(client.create_account("Checking", "Cash").await);

        // Start with $100, spend $150 = -$50
        assert!(
            client
                .create_transaction("2024-01-01", "100.00", "Deposit", Some(1), None)
                .await
        );
        assert!(
            client
                .create_transaction("2024-01-02", "-150.00", "Big purchase", Some(1), Some(1))
                .await
        );

        let (status, body) = client.get("/balances").await;

        assert_eq!(status, StatusCode::OK);
        // Should show negative balance
        assert!(
            body.contains("-$50.00") || body.contains("-50.00"),
            "Expected negative balance of -$50.00 not found"
        );
    }

    /// Test multiple accounts are shown with correct individual balances.
    #[tokio::test]
    async fn test_multiple_accounts() {
        let client = TestClient::new();

        // Create two accounts
        assert!(client.create_account("Checking", "Cash").await);
        assert!(client.create_account("Savings", "Cash").await);

        // Checking: $500
        assert!(
            client
                .create_transaction("2024-01-01", "500.00", "Paycheck", Some(1), None)
                .await
        );

        // Savings: $1000
        assert!(
            client
                .create_transaction("2024-01-01", "1000.00", "Initial deposit", Some(2), None)
                .await
        );

        let (status, body) = client.get("/balances").await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("Checking"));
        assert!(body.contains("Savings"));
        // Check both balances appear
        assert!(body.contains("500.00"), "Checking balance not found");
        assert!(
            body.contains("1,000.00") || body.contains("1000.00"),
            "Savings balance not found"
        );
    }
}

// =============================================================================
// Position Tests
// =============================================================================

mod positions {
    use super::*;

    /// Test positions page loads with empty database.
    #[tokio::test]
    async fn test_positions_empty_db() {
        let client = TestClient::new();
        let (status, body) = client.get("/trading/positions").await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("Positions"));
    }

    /// Test position is correctly computed from buy activities.
    #[tokio::test]
    async fn test_position_from_buys() {
        let client = TestClient::new();

        // Buy 10 shares of AAPL at $150 each
        assert!(
            client
                .create_trading_activity("2024-01-01", "AAPL", "BUY", "10", "150.00")
                .await
        );

        let (status, body) = client.get("/trading/positions").await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("AAPL"), "Symbol AAPL not found");
        // Should show 10 shares
        assert!(
            body.contains(">10<")
                || body.contains(">10.0<")
                || body.contains("10 shares")
                || body.contains(">10</"),
            "Expected 10 shares not found in response"
        );
    }

    /// Test position after partial sell.
    #[tokio::test]
    async fn test_position_after_partial_sell() {
        let client = TestClient::new();

        // Buy 10 shares at $100
        assert!(
            client
                .create_trading_activity("2024-01-01", "MSFT", "BUY", "10", "100.00")
                .await
        );

        // Sell 3 shares at $120
        assert!(
            client
                .create_trading_activity("2024-02-01", "MSFT", "SELL", "3", "120.00")
                .await
        );

        let (status, body) = client.get("/trading/positions").await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("MSFT"));
        // Should show 7 shares remaining
        assert!(
            body.contains(">7<")
                || body.contains(">7.0<")
                || body.contains("7 shares")
                || body.contains(">7</"),
            "Expected 7 shares remaining not found"
        );
    }

    /// Test closed position (fully sold) appears on closed positions page.
    #[tokio::test]
    async fn test_closed_position() {
        let client = TestClient::new();

        // Buy 5 shares
        assert!(
            client
                .create_trading_activity("2024-01-01", "GOOG", "BUY", "5", "100.00")
                .await
        );

        // Sell all 5 shares
        assert!(
            client
                .create_trading_activity("2024-03-01", "GOOG", "SELL", "5", "120.00")
                .await
        );

        // Should NOT appear in open positions
        let (status, _body) = client.get("/trading/positions").await;
        assert_eq!(status, StatusCode::OK);
        // GOOG should not be in the open positions list (or should show 0)

        // Should appear in closed positions
        let (status, body) = client.get("/trading/positions/closed").await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.contains("GOOG"),
            "GOOG should appear in closed positions"
        );
    }
}

// =============================================================================
// Analytics API Tests (JSON endpoints for echarts)
// =============================================================================

mod analytics {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct CategorySpending {
        category: String,
        amount_cents: i64,
    }

    /// Test spending by category with empty database returns empty array.
    #[tokio::test]
    async fn test_spending_by_category_empty() {
        let client = TestClient::new();
        let (status, body) = client.get("/api/analytics/spending-by-category").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "[]");
    }

    /// Test spending by category correctly aggregates transactions.
    #[tokio::test]
    async fn test_spending_by_category_aggregation() {
        let client = TestClient::new();

        // Create transactions in "Food & Dining" category (id=1)
        // Note: negative amounts are expenses
        assert!(
            client
                .create_transaction("2024-01-01", "-50.00", "Lunch", None, Some(1))
                .await
        );
        assert!(
            client
                .create_transaction("2024-01-02", "-30.00", "Coffee", None, Some(1))
                .await
        );

        // Create transaction in "Transportation" category (id=2)
        assert!(
            client
                .create_transaction("2024-01-03", "-20.00", "Bus fare", None, Some(2))
                .await
        );

        let (status, parsed): (_, Option<Vec<CategorySpending>>) =
            client.get_json("/api/analytics/spending-by-category").await;

        assert_eq!(status, StatusCode::OK);
        let data = parsed.expect("Failed to parse JSON response");

        // Find Food & Dining category
        let food = data.iter().find(|c| c.category == "Food & Dining");
        assert!(food.is_some(), "Food & Dining category not found");
        assert_eq!(
            food.unwrap().amount_cents,
            -8000,
            "Food & Dining should total -$80.00 (-8000 cents)"
        );

        // Find Transportation category
        let transport = data.iter().find(|c| c.category == "Transportation");
        assert!(transport.is_some(), "Transportation category not found");
        assert_eq!(
            transport.unwrap().amount_cents,
            -2000,
            "Transportation should total -$20.00 (-2000 cents)"
        );
    }

    /// Test date filtering returns only transactions in range.
    #[tokio::test]
    async fn test_spending_by_category_date_filter() {
        let client = TestClient::new();

        // January transaction
        assert!(
            client
                .create_transaction("2024-01-15", "-50.00", "January expense", None, Some(1))
                .await
        );

        // March transaction
        assert!(
            client
                .create_transaction("2024-03-15", "-70.00", "March expense", None, Some(1))
                .await
        );

        // Query only January
        let (status, parsed): (_, Option<Vec<CategorySpending>>) = client
            .get_json("/api/analytics/spending-by-category?from_date=2024-01-01&to_date=2024-01-31")
            .await;

        assert_eq!(status, StatusCode::OK);
        let data = parsed.expect("Failed to parse JSON");

        // Should only have January's -$50.00
        let total: i64 = data.iter().map(|c| c.amount_cents).sum();
        assert_eq!(total, -5000, "Should only include January expense");
    }

    /// Test monthly summary returns correct monthly totals.
    #[tokio::test]
    async fn test_monthly_summary() {
        let client = TestClient::new();

        // January: -$100
        assert!(
            client
                .create_transaction("2024-01-05", "-100.00", "Rent", None, Some(3))
                .await
        );

        // February: -$150
        assert!(
            client
                .create_transaction("2024-02-05", "-150.00", "Rent", None, Some(3))
                .await
        );

        let (status, body) = client.get("/api/analytics/monthly-summary").await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("2024-01"));
        assert!(body.contains("2024-02"));
    }

    /// Test sankey diagram data structure.
    #[tokio::test]
    async fn test_flow_sankey_structure() {
        let client = TestClient::new();

        // Add income
        assert!(
            client
                .create_transaction("2024-01-01", "1000.00", "Salary", None, Some(8))
                .await
        );

        // Add expenses
        assert!(
            client
                .create_transaction("2024-01-05", "-300.00", "Rent", None, Some(3))
                .await
        );
        assert!(
            client
                .create_transaction("2024-01-10", "-100.00", "Food", None, Some(1))
                .await
        );

        let (status, body) = client.get("/api/analytics/flow-sankey").await;

        assert_eq!(status, StatusCode::OK);
        // Sankey should have nodes and links
        assert!(body.contains("\"nodes\""), "Sankey should have nodes");
        assert!(body.contains("\"links\""), "Sankey should have links");
    }
}

// =============================================================================
// Position Chart API Tests
// =============================================================================

mod position_chart {
    use super::*;

    /// Test position chart API returns data for existing position.
    #[tokio::test]
    async fn test_position_chart_data() {
        let client = TestClient::new();

        // Create a position
        assert!(
            client
                .create_trading_activity("2024-01-01", "NVDA", "BUY", "5", "500.00")
                .await
        );

        let (status, body) = client.get("/api/positions/NVDA/chart").await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("\"symbol\":\"NVDA\""));
    }

    /// Test position chart API for non-existent symbol.
    #[tokio::test]
    async fn test_position_chart_nonexistent() {
        let client = TestClient::new();

        let (status, body) = client.get("/api/positions/DOESNOTEXIST/chart").await;

        assert_eq!(status, StatusCode::OK);
        // Should still return valid JSON, just with no trade data
        assert!(body.contains("\"symbol\":\"DOESNOTEXIST\""));
    }
}

// =============================================================================
// Edge Cases
// =============================================================================

mod edge_cases {
    use super::*;

    /// Test unicode in transaction descriptions.
    #[tokio::test]
    async fn test_unicode_descriptions() {
        let client = TestClient::new();

        // Create transactions with unicode
        assert!(
            client
                .create_transaction("2024-01-01", "-50.00", "東京レストラン", None, Some(1))
                .await
        );
        assert!(
            client
                .create_transaction("2024-01-02", "-30.00", "Café François ☕", None, Some(1))
                .await
        );

        // Verify transactions page loads
        let (status, _body) = client.get("/transactions").await;
        assert_eq!(status, StatusCode::OK);

        // Verify analytics still work
        let (status, _) = client.get("/api/analytics/spending-by-category").await;
        assert_eq!(status, StatusCode::OK);
    }

    /// Test fractional shares in trading positions.
    #[tokio::test]
    async fn test_fractional_shares() {
        let client = TestClient::new();

        // Buy 2.5 shares
        assert!(
            client
                .create_trading_activity("2024-01-01", "VTI", "BUY", "2.5", "200.00")
                .await
        );

        // Buy 0.333 more shares
        assert!(
            client
                .create_trading_activity("2024-02-01", "VTI", "BUY", "0.333", "210.00")
                .await
        );

        let (status, body) = client.get("/trading/positions").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("VTI"));
        // Total should be 2.833 shares
        assert!(
            body.contains("2.833") || body.contains("2.83"),
            "Expected fractional total ~2.833 shares"
        );
    }

    /// Test balance with transactions that exactly cancel out.
    #[tokio::test]
    async fn test_zero_balance() {
        let client = TestClient::new();

        assert!(client.create_account("Temp", "Cash").await);

        // Deposit and withdraw same amount
        assert!(
            client
                .create_transaction("2024-01-01", "500.00", "In", Some(1), None)
                .await
        );
        assert!(
            client
                .create_transaction("2024-01-02", "-500.00", "Out", Some(1), None)
                .await
        );

        let (status, body) = client.get("/balances").await;
        assert_eq!(status, StatusCode::OK);
        // Balance should be $0.00
        assert!(
            body.contains("$0.00") || body.contains("0.00"),
            "Expected zero balance"
        );
    }

    /// Test very large monetary amounts.
    #[tokio::test]
    async fn test_large_amounts() {
        let client = TestClient::new();

        assert!(client.create_account("Trust Fund", "Cash").await);

        // $1 million deposit
        assert!(
            client
                .create_transaction("2024-01-01", "1000000.00", "Inheritance", Some(1), None)
                .await
        );

        let (status, body) = client.get("/balances").await;
        assert_eq!(status, StatusCode::OK);
        // Should show $1,000,000.00
        assert!(
            body.contains("1,000,000") || body.contains("1000000"),
            "Large amount not displayed correctly"
        );
    }

    /// Test transaction with no category appears as uncategorized in analytics.
    #[tokio::test]
    async fn test_uncategorized_transaction() {
        let client = TestClient::new();

        // Create transaction without category
        assert!(
            client
                .create_transaction("2024-01-01", "-100.00", "Mystery expense", None, None)
                .await
        );

        let (status, body) = client.get("/api/analytics/spending-by-category").await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.contains("Uncategorized"),
            "Uncategorized spending should appear"
        );
    }

    /// Test empty date range returns no data.
    #[tokio::test]
    async fn test_empty_date_range() {
        let client = TestClient::new();

        // Create transaction in 2024
        assert!(
            client
                .create_transaction("2024-06-15", "-50.00", "Expense", None, Some(1))
                .await
        );

        // Query for 2023 - should return empty
        let (status, body) = client
            .get("/api/analytics/spending-by-category?from_date=2023-01-01&to_date=2023-12-31")
            .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body, "[]",
            "Should return empty array for date range with no data"
        );
    }
}

// =============================================================================
// Health Check
// =============================================================================

mod health {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        let client = TestClient::new();
        let (status, body) = client.get("/health").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "OK");
    }
}

//! Integration tests for authentication middleware.

mod common;

use axum::http::StatusCode;
use common::TestClient;
use solvency::config::AuthMode;

// A valid Argon2 hash for the password "testpass123"
// Generated with: echo -n 'testpass123' | argon2 somesalt -id -e
const TEST_PASSWORD_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$rJQH0emBAuPPLwwjO9XGAN1BANE";

/// Create a test client with password authentication enabled.
fn auth_client() -> TestClient {
    TestClient::with_auth_mode(AuthMode::Password(TEST_PASSWORD_HASH.to_string()))
}

// =============================================================================
// Tests for protected HTML pages (should redirect to /login)
// =============================================================================

/// Test that home page redirects to login when auth is required.
#[tokio::test]
async fn test_home_requires_auth() {
    let client = auth_client();
    let (status, body) = client.get_with_auth("/").await;

    // Should redirect to login
    assert!(
        status == StatusCode::SEE_OTHER || body.contains("login"),
        "Home page should redirect to login when unauthenticated"
    );
}

/// Test that transactions page redirects to login when auth is required.
#[tokio::test]
async fn test_transactions_requires_auth() {
    let client = auth_client();
    let (status, body) = client.get_with_auth("/transactions").await;

    assert!(
        status == StatusCode::SEE_OTHER || body.contains("login"),
        "Transactions page should redirect to login"
    );
}

/// Test that balances page redirects to login when auth is required.
#[tokio::test]
async fn test_balances_requires_auth() {
    let client = auth_client();
    let (status, body) = client.get_with_auth("/balances").await;

    assert!(
        status == StatusCode::SEE_OTHER || body.contains("login"),
        "Balances page should redirect to login"
    );
}

/// Test that accounts page redirects to login when auth is required.
#[tokio::test]
async fn test_accounts_requires_auth() {
    let client = auth_client();
    let (status, body) = client.get_with_auth("/accounts").await;

    assert!(
        status == StatusCode::SEE_OTHER || body.contains("login"),
        "Accounts page should redirect to login"
    );
}

/// Test that categories page redirects to login when auth is required.
#[tokio::test]
async fn test_categories_requires_auth() {
    let client = auth_client();
    let (status, body) = client.get_with_auth("/categories").await;

    assert!(
        status == StatusCode::SEE_OTHER || body.contains("login"),
        "Categories page should redirect to login"
    );
}

/// Test that trading positions page redirects to login when auth is required.
#[tokio::test]
async fn test_positions_requires_auth() {
    let client = auth_client();
    let (status, body) = client.get_with_auth("/trading/positions").await;

    assert!(
        status == StatusCode::SEE_OTHER || body.contains("login"),
        "Positions page should redirect to login"
    );
}

/// Test that analytics page redirects to login when auth is required.
#[tokio::test]
async fn test_analytics_requires_auth() {
    let client = auth_client();
    let (status, body) = client.get_with_auth("/analytics").await;

    assert!(
        status == StatusCode::SEE_OTHER || body.contains("login"),
        "Analytics page should redirect to login"
    );
}

// =============================================================================
// Tests for API endpoints (should return 401)
// =============================================================================

/// Test that spending-by-category API returns 401 when auth is required.
#[tokio::test]
async fn test_api_spending_by_category_requires_auth() {
    let client = auth_client();
    let (status, _) = client
        .get_with_auth("/api/analytics/spending-by-category")
        .await;

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "API should return 401 when unauthenticated"
    );
}

/// Test that monthly-summary API returns 401 when auth is required.
#[tokio::test]
async fn test_api_monthly_summary_requires_auth() {
    let client = auth_client();
    let (status, _) = client.get_with_auth("/api/analytics/monthly-summary").await;

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "API should return 401 when unauthenticated"
    );
}

/// Test that flow-sankey API returns 401 when auth is required.
#[tokio::test]
async fn test_api_flow_sankey_requires_auth() {
    let client = auth_client();
    let (status, _) = client.get_with_auth("/api/analytics/flow-sankey").await;

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "API should return 401 when unauthenticated"
    );
}

/// Test that position chart API returns 401 when auth is required.
#[tokio::test]
async fn test_api_position_chart_requires_auth() {
    let client = auth_client();
    let (status, _) = client.get_with_auth("/api/positions/AAPL/chart").await;

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "API should return 401 when unauthenticated"
    );
}

// =============================================================================
// Tests for publicly accessible endpoints
// =============================================================================

/// Test that health endpoint is accessible without authentication.
#[tokio::test]
async fn test_health_public() {
    let client = auth_client();
    let (status, body) = client.get_with_auth("/health").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "OK");
}

/// Test that login page is accessible without authentication.
#[tokio::test]
async fn test_login_page_public() {
    let client = auth_client();
    let (status, body) = client.get_with_auth("/login").await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("password") || body.contains("Login"),
        "Login page should be accessible"
    );
}

// =============================================================================
// Tests for unauthenticated mode (no password set)
// =============================================================================

/// Test that pages are accessible when no password is set.
#[tokio::test]
async fn test_no_auth_mode_allows_access() {
    let client = TestClient::new(); // Uses AuthMode::Unauthenticated
    let (status, _) = client.get_with_auth("/").await;

    assert_eq!(
        status,
        StatusCode::OK,
        "Home page should be accessible without auth when no password is set"
    );
}

/// Test that API endpoints are accessible when no password is set.
#[tokio::test]
async fn test_no_auth_mode_allows_api_access() {
    let client = TestClient::new();
    let (status, _) = client
        .get_with_auth("/api/analytics/spending-by-category")
        .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "API should be accessible without auth when no password is set"
    );
}

//! Miscellaneous integration tests (unicode, health check).

mod common;

use axum::http::StatusCode;
use common::TestClient;

/// Test health endpoint.
#[tokio::test]
async fn test_health_endpoint() {
    let client = TestClient::new();
    let (status, body) = client.get("/health").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "OK");
}

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

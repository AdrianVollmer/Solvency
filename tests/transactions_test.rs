//! Integration tests for transaction search and filtering.

mod common;

use axum::http::StatusCode;
use common::TestClient;

/// Searching transactions with an empty category_id (from the "All Categories"
/// select option) must not return 400.
#[tokio::test]
async fn test_search_with_empty_category_id() {
    let client = TestClient::new();

    // The HTML form sends category_id="" when "All Categories" is selected.
    let (status, _) = client
        .get("/transactions/table?search=test&category_id=")
        .await;
    assert_eq!(status, StatusCode::OK);
}

/// Searching with a text term returns only matching transactions.
#[tokio::test]
async fn test_search_filters_by_description() {
    let client = TestClient::new();

    client
        .create_transaction("2024-01-01", "-20.00", "Grocery Store", None, None)
        .await;
    client
        .create_transaction("2024-01-02", "-50.00", "Electric Bill", None, None)
        .await;

    let (status, body) = client
        .get("/transactions/table?search=Grocery&from_date=2024-01-01&to_date=2024-12-31")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Grocery Store"));
    assert!(!body.contains("Electric Bill"));
}

/// Filtering by category_id returns only transactions in that category.
#[tokio::test]
async fn test_filter_by_category_id() {
    let client = TestClient::new();

    // category_id 1 is the default "Uncategorized" created by migrations
    client
        .create_transaction("2024-03-01", "-10.00", "Cat-1 item", None, Some(1))
        .await;

    let (status, body) = client
        .get("/transactions/table?category_id=1&from_date=2024-01-01&to_date=2024-12-31")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Cat-1 item"));
}

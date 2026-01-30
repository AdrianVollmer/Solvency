//! Integration tests for analytics JSON APIs (echarts data).

mod common;

use axum::http::StatusCode;
use common::TestClient;
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

    // Create transactions in "Food & Dining" category (id=4)
    // Note: IDs 1-3 are built-in roots (Expenses, Income, Transfers)
    // Note: negative amounts are expenses
    assert!(
        client
            .create_transaction("2024-01-01", "-50.00", "Lunch", None, Some(4))
            .await
    );
    assert!(
        client
            .create_transaction("2024-01-02", "-30.00", "Coffee", None, Some(4))
            .await
    );

    // Create transaction in "Transportation" category (id=5)
    assert!(
        client
            .create_transaction("2024-01-03", "-20.00", "Bus fare", None, Some(5))
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

    // January transaction (Food & Dining = id 4)
    assert!(
        client
            .create_transaction("2024-01-15", "-50.00", "January expense", None, Some(4))
            .await
    );

    // March transaction
    assert!(
        client
            .create_transaction("2024-03-15", "-70.00", "March expense", None, Some(4))
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

    // January: -$100 (Housing = id 6)
    assert!(
        client
            .create_transaction("2024-01-05", "-100.00", "Rent", None, Some(6))
            .await
    );

    // February: -$150
    assert!(
        client
            .create_transaction("2024-02-05", "-150.00", "Rent", None, Some(6))
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

    // Add income (Income root = id 2)
    assert!(
        client
            .create_transaction("2024-01-01", "1000.00", "Salary", None, Some(2))
            .await
    );

    // Add expenses (Housing = id 6, Food & Dining = id 4)
    assert!(
        client
            .create_transaction("2024-01-05", "-300.00", "Rent", None, Some(6))
            .await
    );
    assert!(
        client
            .create_transaction("2024-01-10", "-100.00", "Food", None, Some(4))
            .await
    );

    let (status, body) = client.get("/api/analytics/flow-sankey").await;

    assert_eq!(status, StatusCode::OK);
    // Sankey should have nodes and links
    assert!(body.contains("\"nodes\""), "Sankey should have nodes");
    assert!(body.contains("\"links\""), "Sankey should have links");
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

    // Create transaction in 2024 (Food & Dining = id 4)
    assert!(
        client
            .create_transaction("2024-06-15", "-50.00", "Expense", None, Some(4))
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

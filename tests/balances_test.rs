//! Integration tests for balance calculations.

mod common;

use axum::http::StatusCode;
use common::TestClient;

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
        body.contains("-\u{2060}$50.00") || body.contains("-50.00"),
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

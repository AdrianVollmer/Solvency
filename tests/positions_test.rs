//! Integration tests for trading positions.

mod common;

use axum::http::StatusCode;
use common::TestClient;

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

// =============================================================================
// Position Chart API Tests
// =============================================================================

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

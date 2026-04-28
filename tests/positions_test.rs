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
// XIRR Tests
// =============================================================================

/// With only buy activities and no market data all cash flows are negative,
/// so XIRR cannot be computed and the stat does not appear on the positions page.
#[tokio::test]
async fn test_positions_page_no_xirr_with_only_buys() {
    let client = TestClient::new();

    assert!(
        client
            .create_trading_activity("2024-01-01", "MSFT", "BUY", "5", "200.00")
            .await
    );

    let (status, body) = client.get("/trading/positions").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !body.contains("Portfolio XIRR"),
        "XIRR should not appear when it cannot be computed"
    );
}

/// A partial sell creates both negative and positive cash flows, allowing XIRR
/// to converge. The stat should appear on the positions page.
#[tokio::test]
async fn test_positions_page_shows_xirr_with_partial_sell() {
    let client = TestClient::new();

    assert!(
        client
            .create_trading_activity("2023-01-01", "AAPL", "BUY", "10", "100.00")
            .await
    );
    assert!(
        client
            .create_trading_activity("2024-01-01", "AAPL", "SELL", "5", "150.00")
            .await
    );

    let (status, body) = client.get("/trading/positions").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Portfolio XIRR"),
        "Portfolio XIRR stat should appear when XIRR is computable"
    );
}

/// Without real market data the remaining open position has no current value,
/// so the XIRR is marked incomplete and the warning tooltip is rendered.
#[tokio::test]
async fn test_positions_page_xirr_incomplete_without_market_data() {
    let client = TestClient::new();

    assert!(
        client
            .create_trading_activity("2023-01-01", "AAPL", "BUY", "10", "100.00")
            .await
    );
    assert!(
        client
            .create_trading_activity("2024-01-01", "AAPL", "SELL", "5", "150.00")
            .await
    );

    let (status, body) = client.get("/trading/positions").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("approximated prices"),
        "Incomplete XIRR warning tooltip should appear when market data is unavailable"
    );
}

/// Closed positions page shows Realized G/L as the hero stat.
#[tokio::test]
async fn test_closed_positions_hero_shows_realized_gl() {
    let client = TestClient::new();

    assert!(
        client
            .create_trading_activity("2024-01-01", "GOOG", "BUY", "10", "100.00")
            .await
    );
    assert!(
        client
            .create_trading_activity("2024-06-01", "GOOG", "SELL", "10", "120.00")
            .await
    );

    let (status, body) = client.get("/trading/positions/closed").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Realized G/L"),
        "Realized G/L hero should appear on closed positions page"
    );
}

/// XIRR appears as a secondary stat on the closed positions page when a full
/// buy-sell cycle exists and all cash flows are known.
#[tokio::test]
async fn test_closed_positions_shows_xirr() {
    let client = TestClient::new();

    assert!(
        client
            .create_trading_activity("2023-01-01", "TSLA", "BUY", "10", "100.00")
            .await
    );
    assert!(
        client
            .create_trading_activity("2024-01-01", "TSLA", "SELL", "10", "110.00")
            .await
    );

    let (status, body) = client.get("/trading/positions/closed").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("XIRR"),
        "XIRR stat should appear on closed positions page"
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

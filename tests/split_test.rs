//! Integration tests for stock split handling.
//!
//! Splits adjust the quantity and unit_price of prior BUY/SELL activities
//! at creation time. When a BUY/SELL is created before an existing split,
//! its values are adjusted too. Deleting a split reverses the adjustments.

mod common;

use common::TestClient;
use solvency::models::TradingActivityType;

// =========================================================================
// Basic split application
// =========================================================================

/// Creating a SPLIT adjusts prior BUY quantities and prices.
#[tokio::test]
async fn test_split_adjusts_prior_buys() {
    let client = TestClient::new();

    // BUY 100 shares at $300
    assert!(
        client
            .create_trading_activity("2024-01-01", "AAPL", "BUY", "100", "300.00")
            .await
    );

    // 2:1 split
    assert!(
        client
            .create_trading_activity("2024-06-15", "AAPL", "SPLIT", "2", "")
            .await
    );

    let activities = client.get_activities_for_symbol("AAPL");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .expect("BUY not found");

    // quantity doubled, price halved
    assert_eq!(buy.quantity, Some(200.0));
    assert_eq!(buy.unit_price_cents, Some(15000)); // $150.00
}

/// Creating a SPLIT adjusts prior SELL quantities and prices.
#[tokio::test]
async fn test_split_adjusts_prior_sells() {
    let client = TestClient::new();

    // BUY 100 shares at $300
    assert!(
        client
            .create_trading_activity("2024-01-01", "AAPL", "BUY", "100", "300.00")
            .await
    );

    // SELL 20 shares at $350
    assert!(
        client
            .create_trading_activity("2024-03-01", "AAPL", "SELL", "20", "350.00")
            .await
    );

    // 2:1 split
    assert!(
        client
            .create_trading_activity("2024-06-15", "AAPL", "SPLIT", "2", "")
            .await
    );

    let activities = client.get_activities_for_symbol("AAPL");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .expect("BUY not found");
    let sell = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Sell)
        .expect("SELL not found");

    assert_eq!(buy.quantity, Some(200.0));
    assert_eq!(buy.unit_price_cents, Some(15000));
    assert_eq!(sell.quantity, Some(40.0));
    assert_eq!(sell.unit_price_cents, Some(17500)); // $175.00
}

/// A BUY after the split date is NOT adjusted.
#[tokio::test]
async fn test_buy_after_split_not_adjusted() {
    let client = TestClient::new();

    // BUY before split
    assert!(
        client
            .create_trading_activity("2024-01-01", "AAPL", "BUY", "100", "300.00")
            .await
    );

    // 2:1 split
    assert!(
        client
            .create_trading_activity("2024-06-15", "AAPL", "SPLIT", "2", "")
            .await
    );

    // BUY after split – should NOT be adjusted
    assert!(
        client
            .create_trading_activity("2024-07-01", "AAPL", "BUY", "50", "150.00")
            .await
    );

    let activities = client.get_activities_for_symbol("AAPL");
    let post_split_buy = activities
        .iter()
        .filter(|a| a.activity_type == TradingActivityType::Buy)
        .last()
        .expect("post-split BUY not found");

    assert_eq!(post_split_buy.quantity, Some(50.0));
    assert_eq!(post_split_buy.unit_price_cents, Some(15000));
}

// =========================================================================
// BUY/SELL before existing split
// =========================================================================

/// A new BUY dated before an existing SPLIT gets adjusted.
#[tokio::test]
async fn test_new_buy_before_existing_split() {
    let client = TestClient::new();

    // Create the split first
    assert!(
        client
            .create_trading_activity("2024-06-15", "TSLA", "SPLIT", "3", "")
            .await
    );

    // Now add a BUY dated before the split
    assert!(
        client
            .create_trading_activity("2024-01-01", "TSLA", "BUY", "10", "600.00")
            .await
    );

    let activities = client.get_activities_for_symbol("TSLA");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .expect("BUY not found");

    // 3:1 split applied: 10 * 3 = 30 shares, $600 / 3 = $200
    assert_eq!(buy.quantity, Some(30.0));
    assert_eq!(buy.unit_price_cents, Some(20000));
}

// =========================================================================
// Multiple splits
// =========================================================================

/// Two splits for the same symbol both adjust a prior BUY.
#[tokio::test]
async fn test_multiple_splits_applied() {
    let client = TestClient::new();

    // BUY 10 shares at $600
    assert!(
        client
            .create_trading_activity("2024-01-01", "TSLA", "BUY", "10", "600.00")
            .await
    );

    // First split: 2:1
    assert!(
        client
            .create_trading_activity("2024-06-01", "TSLA", "SPLIT", "2", "")
            .await
    );

    // Second split: 3:1
    assert!(
        client
            .create_trading_activity("2024-09-01", "TSLA", "SPLIT", "3", "")
            .await
    );

    let activities = client.get_activities_for_symbol("TSLA");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .expect("BUY not found");

    // 10 * 2 * 3 = 60 shares, $600 / 2 / 3 = $100
    assert_eq!(buy.quantity, Some(60.0));
    assert_eq!(buy.unit_price_cents, Some(10000));
}

/// A BUY inserted between two existing splits gets only the later split applied.
#[tokio::test]
async fn test_buy_between_two_splits() {
    let client = TestClient::new();

    // First split: 2:1
    assert!(
        client
            .create_trading_activity("2024-03-01", "GOOG", "SPLIT", "2", "")
            .await
    );

    // Second split: 5:1
    assert!(
        client
            .create_trading_activity("2024-09-01", "GOOG", "SPLIT", "5", "")
            .await
    );

    // BUY between the two splits
    assert!(
        client
            .create_trading_activity("2024-06-01", "GOOG", "BUY", "100", "1000.00")
            .await
    );

    let activities = client.get_activities_for_symbol("GOOG");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .expect("BUY not found");

    // Only the 5:1 split applies (date 2024-09-01 > 2024-06-01).
    // The 2:1 split is before the BUY, so it doesn't apply.
    assert_eq!(buy.quantity, Some(500.0));
    assert_eq!(buy.unit_price_cents, Some(20000)); // $200.00
}

// =========================================================================
// Split isolation (different symbols)
// =========================================================================

/// A split only affects activities for the same symbol.
#[tokio::test]
async fn test_split_does_not_affect_other_symbols() {
    let client = TestClient::new();

    assert!(
        client
            .create_trading_activity("2024-01-01", "AAPL", "BUY", "100", "150.00")
            .await
    );
    assert!(
        client
            .create_trading_activity("2024-01-01", "MSFT", "BUY", "50", "300.00")
            .await
    );

    // Split only AAPL
    assert!(
        client
            .create_trading_activity("2024-06-15", "AAPL", "SPLIT", "4", "")
            .await
    );

    let msft = client.get_activities_for_symbol("MSFT");
    let msft_buy = &msft[0];

    // MSFT should be untouched
    assert_eq!(msft_buy.quantity, Some(50.0));
    assert_eq!(msft_buy.unit_price_cents, Some(30000));
}

// =========================================================================
// Reverse split
// =========================================================================

/// A reverse split (ratio < 1) reduces quantity and increases price.
#[tokio::test]
async fn test_reverse_split() {
    let client = TestClient::new();

    assert!(
        client
            .create_trading_activity("2024-01-01", "GE", "BUY", "800", "10.00")
            .await
    );

    // 1:8 reverse split (ratio = 0.125)
    assert!(
        client
            .create_trading_activity("2024-06-15", "GE", "SPLIT", "0.125", "")
            .await
    );

    let activities = client.get_activities_for_symbol("GE");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .expect("BUY not found");

    // 800 * 0.125 = 100, $10 / 0.125 = $80
    assert_eq!(buy.quantity, Some(100.0));
    assert_eq!(buy.unit_price_cents, Some(8000));
}

// =========================================================================
// Delete split reversal
// =========================================================================

/// Deleting a split restores original quantities and prices.
#[tokio::test]
async fn test_delete_split_reverses_adjustments() {
    let client = TestClient::new();

    assert!(
        client
            .create_trading_activity("2024-01-01", "AAPL", "BUY", "100", "300.00")
            .await
    );

    assert!(
        client
            .create_trading_activity("2024-06-15", "AAPL", "SPLIT", "2", "")
            .await
    );

    // Verify adjusted
    let activities = client.get_activities_for_symbol("AAPL");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .unwrap();
    assert_eq!(buy.quantity, Some(200.0));

    // Delete the split
    let split = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Split)
        .unwrap();
    assert!(client.delete_trading_activity(split.id).await);

    // BUY should be restored to original values
    let activities = client.get_activities_for_symbol("AAPL");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .expect("BUY should still exist");

    assert_eq!(buy.quantity, Some(100.0));
    assert_eq!(buy.unit_price_cents, Some(30000));
}

/// Deleting one of two splits only reverses that split's effect.
#[tokio::test]
async fn test_delete_first_of_two_splits() {
    let client = TestClient::new();

    // BUY 10 at $600
    assert!(
        client
            .create_trading_activity("2024-01-01", "TSLA", "BUY", "10", "600.00")
            .await
    );

    // Split A: 2:1
    assert!(
        client
            .create_trading_activity("2024-06-01", "TSLA", "SPLIT", "2", "")
            .await
    );

    // Split B: 3:1
    assert!(
        client
            .create_trading_activity("2024-09-01", "TSLA", "SPLIT", "3", "")
            .await
    );

    // Currently 10 * 2 * 3 = 60 at $100
    let activities = client.get_activities_for_symbol("TSLA");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .unwrap();
    assert_eq!(buy.quantity, Some(60.0));
    assert_eq!(buy.unit_price_cents, Some(10000));

    // Delete split A (the 2:1)
    let split_a = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Split && a.quantity == Some(2.0))
        .unwrap();
    assert!(client.delete_trading_activity(split_a.id).await);

    // Should now be 10 * 3 = 30 at $200
    let activities = client.get_activities_for_symbol("TSLA");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .unwrap();
    assert_eq!(buy.quantity, Some(30.0));
    assert_eq!(buy.unit_price_cents, Some(20000));
}

/// Deleting the second of two splits only reverses that split's effect.
#[tokio::test]
async fn test_delete_second_of_two_splits() {
    let client = TestClient::new();

    // BUY 10 at $600
    assert!(
        client
            .create_trading_activity("2024-01-01", "TSLA", "BUY", "10", "600.00")
            .await
    );

    // Split A: 2:1
    assert!(
        client
            .create_trading_activity("2024-06-01", "TSLA", "SPLIT", "2", "")
            .await
    );

    // Split B: 3:1
    assert!(
        client
            .create_trading_activity("2024-09-01", "TSLA", "SPLIT", "3", "")
            .await
    );

    // Delete split B (the 3:1)
    let activities = client.get_activities_for_symbol("TSLA");
    let split_b = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Split && a.quantity == Some(3.0))
        .unwrap();
    assert!(client.delete_trading_activity(split_b.id).await);

    // Should now be 10 * 2 = 20 at $300
    let activities = client.get_activities_for_symbol("TSLA");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .unwrap();
    assert_eq!(buy.quantity, Some(20.0));
    assert_eq!(buy.unit_price_cents, Some(30000));
}

// =========================================================================
// Position calculation correctness
// =========================================================================

/// Positions are correct after a split has been applied.
#[tokio::test]
async fn test_position_correct_after_split() {
    let client = TestClient::new();

    // BUY 100 at $150
    assert!(
        client
            .create_trading_activity("2024-01-01", "AAPL", "BUY", "100", "150.00")
            .await
    );

    // 4:1 split
    assert!(
        client
            .create_trading_activity("2024-06-15", "AAPL", "SPLIT", "4", "")
            .await
    );

    // BUY 50 more at the new post-split price $37.50
    assert!(
        client
            .create_trading_activity("2024-07-01", "AAPL", "BUY", "50", "37.50")
            .await
    );

    let (status, body) = client.get("/trading/positions").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert!(body.contains("AAPL"));

    // Position: 400 (adjusted) + 50 = 450 shares
    // Cost: $15,000 + $1,875 = $16,875
    assert!(
        body.contains("450") || body.contains("450.0"),
        "Expected 450 shares in position"
    );
}

/// Closed position after split is computed correctly.
#[tokio::test]
async fn test_closed_position_after_split() {
    let client = TestClient::new();

    // BUY 10 at $100
    assert!(
        client
            .create_trading_activity("2024-01-01", "XYZ", "BUY", "10", "100.00")
            .await
    );

    // 2:1 split → 20 shares at $50
    assert!(
        client
            .create_trading_activity("2024-06-01", "XYZ", "SPLIT", "2", "")
            .await
    );

    // SELL all 20 at $60
    assert!(
        client
            .create_trading_activity("2024-07-01", "XYZ", "SELL", "20", "60.00")
            .await
    );

    let (status, body) = client.get("/trading/positions/closed").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert!(
        body.contains("XYZ"),
        "XYZ should appear in closed positions"
    );
}

// =========================================================================
// Same-date BUY not adjusted
// =========================================================================

/// A BUY on the same date as a split is NOT adjusted (strict date < check).
#[tokio::test]
async fn test_buy_on_split_date_not_adjusted() {
    let client = TestClient::new();

    assert!(
        client
            .create_trading_activity("2024-06-15", "AAPL", "BUY", "100", "150.00")
            .await
    );

    assert!(
        client
            .create_trading_activity("2024-06-15", "AAPL", "SPLIT", "2", "")
            .await
    );

    let activities = client.get_activities_for_symbol("AAPL");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .unwrap();

    // Same date → not adjusted
    assert_eq!(buy.quantity, Some(100.0));
    assert_eq!(buy.unit_price_cents, Some(15000));
}

// =========================================================================
// Update scenarios
// =========================================================================

/// Updating a split's ratio re-applies with the new ratio.
#[tokio::test]
async fn test_update_split_ratio() {
    let client = TestClient::new();

    assert!(
        client
            .create_trading_activity("2024-01-01", "AAPL", "BUY", "100", "300.00")
            .await
    );
    assert!(
        client
            .create_trading_activity("2024-06-15", "AAPL", "SPLIT", "2", "")
            .await
    );

    // Verify 2:1 applied
    let activities = client.get_activities_for_symbol("AAPL");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .unwrap();
    assert_eq!(buy.quantity, Some(200.0));

    // Update split to 4:1
    let split = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Split)
        .unwrap();
    assert!(
        client
            .update_trading_activity(split.id, "2024-06-15", "AAPL", "SPLIT", "4", "")
            .await
    );

    let activities = client.get_activities_for_symbol("AAPL");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .unwrap();

    // Now 4:1 applied from the original 100
    assert_eq!(buy.quantity, Some(400.0));
    assert_eq!(buy.unit_price_cents, Some(7500)); // $75.00
}

// =========================================================================
// Split with no quantity is harmless
// =========================================================================

/// A SPLIT with no quantity does nothing.
#[tokio::test]
async fn test_split_without_quantity_is_noop() {
    let client = TestClient::new();

    assert!(
        client
            .create_trading_activity("2024-01-01", "AAPL", "BUY", "100", "150.00")
            .await
    );

    // Split with empty quantity
    assert!(
        client
            .create_trading_activity("2024-06-15", "AAPL", "SPLIT", "", "")
            .await
    );

    let activities = client.get_activities_for_symbol("AAPL");
    let buy = activities
        .iter()
        .find(|a| a.activity_type == TradingActivityType::Buy)
        .unwrap();

    // Unchanged
    assert_eq!(buy.quantity, Some(100.0));
    assert_eq!(buy.unit_price_cents, Some(15000));
}

// =========================================================================
// Multiple buys before split
// =========================================================================

/// Multiple BUY activities before a split are all adjusted.
#[tokio::test]
async fn test_multiple_buys_before_split() {
    let client = TestClient::new();

    assert!(
        client
            .create_trading_activity("2024-01-01", "NVDA", "BUY", "10", "200.00")
            .await
    );
    assert!(
        client
            .create_trading_activity("2024-02-01", "NVDA", "BUY", "5", "220.00")
            .await
    );
    assert!(
        client
            .create_trading_activity("2024-03-01", "NVDA", "BUY", "15", "180.00")
            .await
    );

    // 10:1 split
    assert!(
        client
            .create_trading_activity("2024-06-15", "NVDA", "SPLIT", "10", "")
            .await
    );

    let activities = client.get_activities_for_symbol("NVDA");
    let buys: Vec<_> = activities
        .iter()
        .filter(|a| a.activity_type == TradingActivityType::Buy)
        .collect();

    assert_eq!(buys.len(), 3);
    assert_eq!(buys[0].quantity, Some(100.0)); // 10 * 10
    assert_eq!(buys[0].unit_price_cents, Some(2000)); // $20
    assert_eq!(buys[1].quantity, Some(50.0)); // 5 * 10
    assert_eq!(buys[1].unit_price_cents, Some(2200)); // $22
    assert_eq!(buys[2].quantity, Some(150.0)); // 15 * 10
    assert_eq!(buys[2].unit_price_cents, Some(1800)); // $18
}

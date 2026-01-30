//! Integration tests for CSV import upload with XSRF protection.

mod common;

use axum::http::StatusCode;
use common::TestClient;

const VALID_CSV: &[u8] = b"date,amount,currency,description\n2024-01-15,-42.50,EUR,Groceries\n";

/// Multipart upload without XSRF header must be rejected with 403.
#[tokio::test]
async fn test_import_upload_rejected_without_xsrf() {
    let client = TestClient::new();

    let (status, _) = client
        .post_multipart_without_xsrf("/import/upload", "files", "test.csv", VALID_CSV)
        .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// Multipart upload with valid XSRF header must succeed (303 redirect to wizard).
#[tokio::test]
async fn test_import_upload_succeeds_with_xsrf() {
    let client = TestClient::new();

    let (status, _) = client
        .post_multipart("/import/upload", "files", "test.csv", VALID_CSV)
        .await;

    assert_eq!(status, StatusCode::SEE_OTHER);
}

/// Trading import upload without XSRF header must be rejected with 403.
#[tokio::test]
async fn test_trading_import_upload_rejected_without_xsrf() {
    let client = TestClient::new();

    let csv = b"date,symbol,activity_type,quantity,unit_price,currency\n2024-01-15,AAPL,buy,10,150.00,USD\n";

    let (status, _) = client
        .post_multipart_without_xsrf("/trading/import/upload", "files", "trades.csv", csv)
        .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// Trading import upload with valid XSRF header must succeed.
#[tokio::test]
async fn test_trading_import_upload_succeeds_with_xsrf() {
    let client = TestClient::new();

    let csv = b"date,symbol,activity_type,quantity,unit_price,currency\n2024-01-15,AAPL,buy,10,150.00,USD\n";

    let (status, _) = client
        .post_multipart("/trading/import/upload", "files", "trades.csv", csv)
        .await;

    assert_eq!(status, StatusCode::SEE_OTHER);
}

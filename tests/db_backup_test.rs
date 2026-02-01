//! Integration tests for database export and import endpoints.
//!
//! The export/import handlers use PID-based temp file paths, so tests that
//! exercise these endpoints must not run concurrently within the same process.
//! We serialize them with a shared mutex.

mod common;

use axum::http::StatusCode;
use common::TestClient;
use solvency::db::queries::{accounts, categories, transactions};
use std::sync::LazyLock;
use tokio::sync::Mutex;
use transactions::TransactionFilter;

/// Serializes tests that hit the export/import endpoints (they share temp files
/// keyed by PID, so concurrent runs within one process would collide).
static DB_BACKUP_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// The SQLite file header magic bytes.
const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";

/// Export endpoint returns 200 with valid SQLite bytes.
#[tokio::test]
async fn test_export_returns_valid_sqlite() {
    let _guard = DB_BACKUP_LOCK.lock().await;

    let client = TestClient::new();

    let (status, bytes) = client.get_bytes("/settings/export-database").await;

    assert_eq!(status, StatusCode::OK);
    assert!(bytes.len() >= 16, "response too short to be a SQLite file");
    assert_eq!(&bytes[..16], SQLITE_MAGIC);
}

/// Seed data, export, import into a fresh client, verify data survives.
#[tokio::test]
async fn test_export_import_round_trip() {
    let _guard = DB_BACKUP_LOCK.lock().await;

    let client_a = TestClient::new();

    // Seed data
    assert!(client_a.create_account("Checking", "Cash").await);
    assert!(
        client_a
            .post_form("/categories/create", &[("name", "Groceries")])
            .await
            .0
            == StatusCode::SEE_OTHER
    );
    assert!(
        client_a
            .create_transaction("2024-06-01", "-50.00", "Weekly shop", Some(1), Some(1))
            .await
    );

    // Export
    let (status, exported) = client_a.get_bytes("/settings/export-database").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(&exported[..16], SQLITE_MAGIC);

    // Import into fresh client
    let client_b = TestClient::new();
    let (status, _) = client_b
        .post_multipart(
            "/settings/import-database",
            "file",
            "backup.db",
            &exported,
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // Verify data survived
    let conn = client_b.state().db.get().unwrap();
    let accts = accounts::list_accounts(&conn).unwrap();
    assert_eq!(accts.len(), 1);
    assert_eq!(accts[0].name, "Checking");

    let cats = categories::list_categories(&conn).unwrap();
    assert!(
        cats.iter().any(|c| c.name == "Groceries"),
        "Groceries category not found after import"
    );

    let txns =
        transactions::list_transactions(&conn, &TransactionFilter::default()).unwrap();
    assert_eq!(txns.len(), 1);
    assert_eq!(txns[0].transaction.description, "Weekly shop");
}

/// Import overwrites the existing data in the target database.
#[tokio::test]
async fn test_import_overwrites_existing_data() {
    let _guard = DB_BACKUP_LOCK.lock().await;

    // Client A: seed and export
    let client_a = TestClient::new();
    assert!(client_a.create_account("Savings", "Cash").await);
    assert!(
        client_a
            .create_transaction("2024-01-01", "1000.00", "Initial deposit", Some(1), None)
            .await
    );

    let (_, exported_a) = client_a.get_bytes("/settings/export-database").await;

    // Client B: seed different data
    let client_b = TestClient::new();
    assert!(client_b.create_account("Credit Card", "Cash").await);
    assert!(
        client_b
            .create_transaction("2024-02-01", "-20.00", "Coffee", Some(1), None)
            .await
    );

    // Import A's export into B
    let (status, _) = client_b
        .post_multipart(
            "/settings/import-database",
            "file",
            "backup.db",
            &exported_a,
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // B should now have only A's data
    let conn = client_b.state().db.get().unwrap();
    let accts = accounts::list_accounts(&conn).unwrap();
    assert_eq!(accts.len(), 1);
    assert_eq!(accts[0].name, "Savings");

    let txns =
        transactions::list_transactions(&conn, &TransactionFilter::default()).unwrap();
    assert_eq!(txns.len(), 1);
    assert_eq!(txns[0].transaction.description, "Initial deposit");
}

/// Uploading garbage bytes is rejected.
#[tokio::test]
async fn test_import_invalid_file_rejected() {
    let _guard = DB_BACKUP_LOCK.lock().await;

    let client = TestClient::new();

    let garbage = b"this is not a valid database file at all";
    let (status, body) = client
        .post_multipart(
            "/settings/import-database",
            "file",
            "bad.db",
            garbage,
        )
        .await;

    // Handler tries to execute the bytes as SQL — should fail
    assert!(
        status != StatusCode::OK || body.contains("error") || body.contains("Error"),
        "Expected an error for invalid file, got status={status}"
    );
}

/// Uploading an empty file returns a validation error.
#[tokio::test]
async fn test_import_empty_file_rejected() {
    let _guard = DB_BACKUP_LOCK.lock().await;

    let client = TestClient::new();

    let (status, body) = client
        .post_multipart("/settings/import-database", "file", "empty.db", b"")
        .await;

    // The handler checks `file_bytes.is_empty()` and returns Validation error
    assert!(
        status == StatusCode::UNPROCESSABLE_ENTITY
            || status == StatusCode::BAD_REQUEST
            || body.contains("No file uploaded"),
        "Expected validation error for empty file, got status={status}"
    );
}

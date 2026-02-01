//! Integration tests for the generation-based query cache.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::TestClient;
use http_body_util::BodyExt;
use solvency::db::queries::{accounts, categories, tags};
use solvency::models::{NewAccount, NewCategory, NewTag};
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Direct cache behaviour (no middleware)
// ---------------------------------------------------------------------------

/// Cached reads return the same data without re-querying when the
/// generation has not changed.
#[tokio::test]
async fn test_cached_reads_are_consistent() {
    let client = TestClient::new();

    let cats_1 = client.state().cached_categories().unwrap();
    let cats_2 = client.state().cached_categories().unwrap();
    assert_eq!(cats_1.len(), cats_2.len());

    let tags_1 = client.state().cached_tags().unwrap();
    let tags_2 = client.state().cached_tags().unwrap();
    assert_eq!(tags_1.len(), tags_2.len());

    let accs_1 = client.state().cached_accounts().unwrap();
    let accs_2 = client.state().cached_accounts().unwrap();
    assert_eq!(accs_1.len(), accs_2.len());
}

/// After a direct DB mutation *without* cache invalidation, cached reads
/// return stale data.  After invalidation the fresh data appears.
#[tokio::test]
async fn test_stale_cache_refreshes_after_invalidation() {
    let client = TestClient::new();
    let state = client.state();

    // Warm the cache.
    let before = state.cached_accounts().unwrap();
    assert!(before.is_empty());

    // Mutate the DB directly (bypass handlers / middleware).
    let conn = state.db.get().unwrap();
    accounts::create_account(
        &conn,
        &NewAccount {
            name: "Savings".into(),
            account_type: solvency::models::AccountType::Cash,
        },
    )
    .unwrap();

    // Cache is still stale.
    let stale = state.cached_accounts().unwrap();
    assert!(stale.is_empty(), "cache should still be stale");

    // Invalidate and re-read.
    state.cache.invalidate();
    let fresh = state.cached_accounts().unwrap();
    assert_eq!(fresh.len(), 1);
    assert_eq!(fresh[0].name, "Savings");
}

/// Invalidation affects all cached slots, not just one.
#[tokio::test]
async fn test_invalidation_clears_all_slots() {
    let client = TestClient::new();
    let state = client.state();

    // Warm every slot.
    let _ = state.cached_categories().unwrap();
    let _ = state.cached_categories_with_path().unwrap();
    let _ = state.cached_tags().unwrap();
    let _ = state.cached_accounts().unwrap();
    let _ = state.cached_cash_accounts().unwrap();
    let _ = state.load_settings().unwrap();

    // Insert one of each entity directly.
    let conn = state.db.get().unwrap();
    categories::create_category(
        &conn,
        &NewCategory {
            name: "Food".into(),
            parent_id: None,
            color: "#ff0000".into(),
            icon: "utensils".into(),
        },
    )
    .unwrap();
    tags::create_tag(
        &conn,
        &NewTag {
            name: "urgent".into(),
            color: "#00ff00".into(),
            style: solvency::models::TagStyle::default(),
        },
    )
    .unwrap();
    accounts::create_account(
        &conn,
        &NewAccount {
            name: "Checking".into(),
            account_type: solvency::models::AccountType::Cash,
        },
    )
    .unwrap();

    // Still stale.
    assert!(state.cached_accounts().unwrap().is_empty());

    // Single invalidation refreshes all slots.
    state.cache.invalidate();

    let cats = state.cached_categories().unwrap();
    // Migrations create built-in categories; "Food" is an extra one.
    assert!(
        cats.iter().any(|c| c.name == "Food"),
        "Food category not found after invalidation"
    );

    let tag_list = state.cached_tags().unwrap();
    assert!(
        tag_list.iter().any(|t| t.name == "urgent"),
        "urgent tag not found after invalidation"
    );

    let accs = state.cached_accounts().unwrap();
    assert!(
        accs.iter().any(|a| a.name == "Checking"),
        "Checking account not found after invalidation"
    );
}

// ---------------------------------------------------------------------------
// Middleware integration
// ---------------------------------------------------------------------------

/// Helper: make a request against a specific router and return (status, body).
async fn request(router: axum::Router, method: &str, uri: &str) -> (StatusCode, String) {
    let resp = router
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

/// Helper: POST a form against a specific router.
async fn post_form(router: axum::Router, uri: &str, form: &[(&str, &str)]) -> (StatusCode, String) {
    let body = form
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    let resp = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

/// A successful POST through the cache middleware invalidates the cache,
/// so a subsequent GET sees the new data.
#[tokio::test]
async fn test_middleware_invalidates_on_successful_post() {
    let client = TestClient::new();

    // Warm the accounts cache via a GET.
    let (status, body) = request(client.router_with_cache(), "GET", "/accounts").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains("MyBank"));

    // POST to create an account — middleware should bump generation.
    let (status, _) = post_form(
        client.router_with_cache(),
        "/accounts/create",
        &[("name", "MyBank"), ("account_type", "Cash")],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    // GET again — should see the new account.
    let (status, body) = request(client.router_with_cache(), "GET", "/accounts").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("MyBank"),
        "new account not visible after POST"
    );
}

/// GET requests must not invalidate the cache.
#[tokio::test]
async fn test_get_does_not_invalidate() {
    let client = TestClient::new();
    let state = client.state();

    // Warm cache via the handler path.
    let _ = request(client.router_with_cache(), "GET", "/accounts").await;

    // Insert directly in DB (bypassing middleware).
    let conn = state.db.get().unwrap();
    accounts::create_account(
        &conn,
        &NewAccount {
            name: "Hidden".into(),
            account_type: solvency::models::AccountType::Cash,
        },
    )
    .unwrap();

    // Another GET should NOT cause invalidation — data stays stale.
    let (_, body) = request(client.router_with_cache(), "GET", "/accounts").await;
    assert!(
        !body.contains("Hidden"),
        "GET should not invalidate the cache"
    );
}

/// A POST that results in a client error (e.g. validation failure) should
/// not invalidate the cache.
#[tokio::test]
async fn test_failed_post_does_not_invalidate() {
    let client = TestClient::new();
    let state = client.state();

    // Seed an account and warm the cache.
    let conn = state.db.get().unwrap();
    accounts::create_account(
        &conn,
        &NewAccount {
            name: "Original".into(),
            account_type: solvency::models::AccountType::Cash,
        },
    )
    .unwrap();
    state.cache.invalidate();

    let (_, body) = request(client.router_with_cache(), "GET", "/accounts").await;
    assert!(body.contains("Original"));

    // Insert another account directly (bypassing middleware).
    accounts::create_account(
        &conn,
        &NewAccount {
            name: "Sneaky".into(),
            account_type: solvency::models::AccountType::Cash,
        },
    )
    .unwrap();

    // POST with invalid data — missing required `account_type` field.
    let (status, _) = post_form(
        client.router_with_cache(),
        "/accounts/create",
        &[("name", "Bad")],
    )
    .await;
    // Should be a client error (4xx) — the exact code depends on the handler,
    // but it must NOT be a redirect (success).
    assert_ne!(
        status,
        StatusCode::SEE_OTHER,
        "invalid POST should not succeed"
    );

    // Cache should still be stale — "Sneaky" not visible.
    let (_, body) = request(client.router_with_cache(), "GET", "/accounts").await;
    assert!(
        !body.contains("Sneaky"),
        "failed POST should not invalidate cache"
    );
}

/// The full create-then-view cycle works for tags through the cache.
#[tokio::test]
async fn test_tag_visible_after_creation() {
    let client = TestClient::new();

    // Create a tag through the handler.
    let (status, _) = post_form(
        client.router_with_cache(),
        "/tags/create",
        &[("name", "important"), ("color", "#ff0000")],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    // The tag should appear on the transaction form (which uses cached_tags).
    let (status, body) = request(client.router_with_cache(), "GET", "/transactions/new").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("important"),
        "newly created tag not visible on transaction form"
    );
}

/// The full create-then-view cycle works for categories through the cache.
#[tokio::test]
async fn test_category_visible_after_creation() {
    let client = TestClient::new();

    // Create a category through the handler.
    let (status, _) = post_form(
        client.router_with_cache(),
        "/categories/create",
        &[
            ("name", "Groceries"),
            ("color", "#22c55e"),
            ("icon", "shopping-cart"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    // The category should appear on the manage page (categories tab).
    let (status, body) = request(client.router_with_cache(), "GET", "/manage?tab=categories").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Groceries"),
        "newly created category not visible"
    );
}

# Tests: Missing Handler and Integration Tests

## Priority
High (Tests)

## Location
`src/handlers/*.rs`, `src/db/queries/*.rs`

## Description
The codebase has minimal test coverage. The only tests found are unit tests
in `src/services/csv_parser.rs`. There are no tests for:

- HTTP handlers (endpoints, request/response handling)
- Database query functions
- Integration tests (end-to-end flows)
- Error handling paths

This makes it risky to refactor or add features, as regressions may go
unnoticed.

## Recommendation
Add tests at multiple levels:

1. **Unit tests for database queries:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // Run migrations
        conn
    }

    #[test]
    fn test_create_expense() {
        let conn = setup_test_db();
        // Test expense creation
    }
}
```

2. **Handler tests using Axum's test utilities:**
```rust
#[tokio::test]
async fn test_expenses_index() {
    let app = create_test_app();
    let response = app
        .oneshot(Request::builder().uri("/expenses").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
```

3. **Integration tests for critical flows** like CSV import, expense CRUD,
   and category management.

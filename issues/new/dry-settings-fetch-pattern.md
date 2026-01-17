# DRY: Repetitive Settings Fetching Pattern

## Priority
Low (Maintainability)

## Location
Throughout `src/handlers/*.rs`

## Description
Nearly every handler contains the same boilerplate for fetching settings:

```rust
let settings_map = settings::get_all_settings(&conn)?;
let app_settings = Settings::from_map(settings_map);
```

This pattern appears in:
- `expenses.rs`: 5 times
- `import.rs`: 3 times
- `categories.rs`: 2 times
- `tags.rs`: 2 times
- `rules.rs`: 2 times
- `trading_activities.rs`: 5 times
- And more...

## Recommendation
Create a helper method or implement this as part of an Axum extractor:

Option 1 - Helper function:
```rust
pub fn get_settings(conn: &Connection) -> AppResult<Settings> {
    let settings_map = settings::get_all_settings(conn)?;
    Ok(Settings::from_map(settings_map))
}
```

Option 2 - Custom Axum extractor:
```rust
pub struct AppSettings(pub Settings);

#[async_trait]
impl<S> FromRequestParts<S> for AppSettings
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    // Implementation that fetches settings
}
```

This would allow handlers to simply receive `AppSettings` as a parameter.

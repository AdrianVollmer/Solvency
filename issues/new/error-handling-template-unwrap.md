# Error Handling: Template Rendering Uses `unwrap()`

## Priority
Low (Error Handling)

## Location
Throughout `src/handlers/*.rs`

## Description
All template rendering calls use `unwrap()` which could panic:

```rust
Ok(Html(template.render().unwrap()))
```

This pattern appears in every handler that returns an HTML response. While
Askama templates are compiled at build time and rendering failures are rare,
using `unwrap()` is not idiomatic error handling and could cause the server
to panic in unexpected situations (e.g., memory pressure, I/O errors if
templates use includes).

Examples:
- `handlers/expenses.rs:237`
- `handlers/import.rs:112`
- `handlers/categories.rs:55`
- And many more...

## Recommendation
Use the `?` operator to propagate errors properly:

```rust
Ok(Html(template.render().map_err(|e| {
    AppError::Internal(format!("Template render failed: {}", e))
})?))
```

Or add a helper method:

```rust
trait RenderExt {
    fn render_html(self) -> AppResult<Html<String>>;
}

impl<T: Template> RenderExt for T {
    fn render_html(self) -> AppResult<Html<String>> {
        self.render()
            .map(Html)
            .map_err(|e| AppError::Internal(format!("Template error: {}", e)))
    }
}
```

Then use as: `Ok(template.render_html()?)`

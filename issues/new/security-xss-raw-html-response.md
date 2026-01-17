# Security: Potential XSS in Raw HTML Response

## Priority
High (Security)

## Location
`src/handlers/import.rs:342-349`

## Description
The `update_row_category` handler returns a raw HTML string containing the
category name without proper HTML escaping:

```rust
Ok(Html(format!(
    r#"<span class="text-gray-600 dark:text-gray-400">{}</span>"#,
    if cat_name.is_empty() {
        "Uncategorized"
    } else {
        &cat_name
    }
)))
```

If an attacker manages to create a category with a malicious name containing
JavaScript (e.g., `<script>alert('xss')</script>`), this code would execute
the script when the HTML is rendered.

## Recommendation
Use HTML escaping for the category name. Consider using `askama::MarkupDisplay`
or a dedicated HTML escaping function:

```rust
use askama::Html as AskamaHtml;

let escaped_name = askama::MarkupDisplay::new_unsafe(&cat_name, askama::Html);
```

Or create a small template for this response to leverage Askama's automatic
escaping.

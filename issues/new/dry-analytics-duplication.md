# DRY: Duplicate Analytics Logic

## Priority
Low (Maintainability)

## Location
- `src/handlers/api.rs:37-87`
- `src/services/analytics.rs:58-97`

## Description
The `spending_by_category` function in `api.rs` and the `spending_by_category`
function in `services/analytics.rs` contain nearly identical logic for
aggregating expenses by category:

**api.rs:**
```rust
let mut category_totals: std::collections::HashMap<String, (String, i64)> =
    std::collections::HashMap::new();

for expense in &expense_list {
    let category_name = expense.category_name.clone().unwrap_or_else(|| "Uncategorized".into());
    let color = expense.category_color.clone().unwrap_or_else(|| "#6b7280".into());
    let entry = category_totals.entry(category_name).or_insert((color, 0));
    entry.1 += expense.expense.amount_cents;
}
```

**services/analytics.rs:**
```rust
let mut category_data: HashMap<String, (String, i64, usize)> = HashMap::new();

for expense in expenses {
    let category = expense.category_name.clone().unwrap_or_else(|| "Uncategorized".into());
    let color = expense.category_color.clone().unwrap_or_else(|| "#6b7280".into());
    let entry = category_data.entry(category).or_insert((color, 0, 0));
    entry.1 += expense.expense.amount_cents;
    entry.2 += 1;
}
```

## Recommendation
Consolidate this logic into `services/analytics.rs` and have the API handler
use that service. This ensures consistency and reduces maintenance burden.

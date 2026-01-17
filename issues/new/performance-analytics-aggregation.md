# Performance: Analytics Aggregation in Application Layer

## Priority
Medium (Performance)

## Location
`src/handlers/api.rs:37-87`, `src/handlers/api.rs:89-120`, `src/handlers/api.rs:122-168`

## Description
The analytics API endpoints (`spending_by_category`, `spending_over_time`,
`monthly_summary`) fetch all matching expenses from the database and then
aggregate them in Rust code:

```rust
let expense_list = expenses::list_expenses(&conn, &filter)?;

let mut category_totals: std::collections::HashMap<String, (String, i64)> =
    std::collections::HashMap::new();

for expense in &expense_list {
    // ... aggregate in memory
}
```

For large datasets, this is inefficient as it:
1. Transfers more data than necessary from the database
2. Uses application memory for aggregation
3. Doesn't leverage database indexing for grouping

## Recommendation
Create dedicated SQL queries that perform the aggregation in the database:

```sql
SELECT c.name as category, c.color, SUM(e.amount_cents) as total
FROM expenses e
LEFT JOIN categories c ON e.category_id = c.id
WHERE e.date >= ? AND e.date <= ?
GROUP BY e.category_id
ORDER BY total DESC
```

This would significantly improve performance for users with large expense
histories.

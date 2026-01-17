# Performance: N+1 Query for Expense Tags

## Priority
High (Performance)

## Location
`src/db/queries/expenses.rs:97-99`

## Description
When listing expenses, the code fetches all expenses first, then loops through
each expense to fetch its tags in a separate query:

```rust
let mut expenses: Vec<ExpenseWithRelations> = expense_iter.filter_map(|e| e.ok()).collect();

for expense in &mut expenses {
    expense.tags = get_expense_tags(conn, expense.expense.id)?;
}
```

This creates N+1 queries where N is the number of expenses. For a page with
50 expenses, this means 51 database queries instead of a potential 2.

## Recommendation
Fetch all tags for the relevant expenses in a single query using the expense
IDs, then map them in memory:

```rust
let expense_ids: Vec<i64> = expenses.iter().map(|e| e.expense.id).collect();
let all_tags = get_tags_for_expenses(conn, &expense_ids)?;

// Create a HashMap of expense_id -> Vec<Tag>
let tags_map: HashMap<i64, Vec<Tag>> = /* group all_tags by expense_id */;

for expense in &mut expenses {
    expense.tags = tags_map.remove(&expense.expense.id).unwrap_or_default();
}
```

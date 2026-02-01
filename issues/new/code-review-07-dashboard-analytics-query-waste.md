## Dashboard and analytics load full objects for simple aggregations

**Priority: Performance (High)**

### Dashboard

`src/handlers/dashboard.rs:49-64` fetches all `TransactionWithRelations`
for two entire months (including JOINs to categories, accounts, and a
second query per transaction for tags) just to sum `amount_cents`:

```rust
let this_month_transactions = transactions::list_transactions(&conn, &filter)?;
let total: i64 = this_month_transactions.iter().map(|e| e.transaction.amount_cents).sum();
```

This should be `SELECT COALESCE(SUM(amount_cents), 0) FROM transactions WHERE date >= ?`.

### Analytics endpoints

Every endpoint in `src/handlers/api.rs` follows the same anti-pattern:
load all transactions with full JOINs, then aggregate in Rust code.

- `spending_by_category` (line 80): GROUP BY category in Rust
- `spending_over_time` (line 121): daily SUM in Rust
- `monthly_summary` (line 202): monthly income/expense in Rust
- `flow_sankey` (line 691): 430-line handler doing all aggregation in Rust

For users with tens of thousands of transactions, this is by far
the biggest performance bottleneck.

### Fix

Add dedicated aggregate query functions (`sum_transactions`,
`sum_by_category`, `sum_by_month`, etc.) that use SQL `GROUP BY`
and `SUM()`. Return only the aggregated numbers.

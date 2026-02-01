## `.filter_map(|r| r.ok())` silently drops database errors

**Priority: Correctness (High)**

Approximately 38 query functions use this pattern to collect rows:

```rust
let items = stmt
    .query_map(params, row_to_item)?
    .filter_map(|r| r.ok())   // silently drops row errors
    .collect();
```

If any row fails to deserialize (corrupt data, schema mismatch after
a migration, unexpected NULL), that row is silently missing from the
result. No error, no log, no indication. This can cause:

- Wrong totals (transactions missing from sum)
- Incorrect positions (activities silently dropped)
- Phantom disappearance of records

Affected files (non-exhaustive):

- `src/db/queries/accounts.rs` (lines 26, 43)
- `src/db/queries/transactions.rs` (line 134)
- `src/db/queries/categories.rs` (lines 25, 65, 158, 231)
- `src/db/queries/tags.rs` (lines 23, 53, 79)
- `src/db/queries/trading.rs` (lines 99, 333, 361, 387, 411, ...)

Contrast with `src/db/queries/import.rs:189` which correctly uses:

```rust
.collect::<Result<Vec<_>, _>>()?
```

### Fix

Replace `.filter_map(|r| r.ok())` with `.collect::<Result<Vec<_>, _>>()?`
everywhere. This surfaces deserialization errors instead of hiding them.

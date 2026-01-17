# Performance: Database Connection Per Row in Import

## Priority
Medium (Performance)

## Location
`src/handlers/import.rs:418-421`

## Description
In the `import_rows_background` function, a new database connection is acquired
from the pool for each row being imported:

```rust
for row in pending_rows {
    let conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => continue,
    };
    // ... process row
}
```

While connection pooling mitigates some overhead, acquiring and releasing
connections for each row in a potentially large import is inefficient.

## Recommendation
Acquire a single connection before the loop and reuse it for all rows:

```rust
let conn = match state.db.get() {
    Ok(c) => c,
    Err(_) => return,
};

for row in pending_rows {
    // use conn for all operations
}
```

For very large imports, consider batching the inserts into transactions of
100-1000 rows each for better performance.

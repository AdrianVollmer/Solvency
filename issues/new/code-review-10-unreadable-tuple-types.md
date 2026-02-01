## Opaque tuple types make trading code hard to read and maintain

**Priority: Readability / Aesthetics**

`src/db/queries/trading.rs` defines three type aliases for tuples
used as data carriers:

```rust
// line 12: 6-element tuple
type ActivityRow = (String, String, Option<f64>, Option<i64>, i64, String);

// line 15: 6-element tuple
type ClosedPositionActivityRow = (String, String, Option<f64>, Option<i64>, String, String);

// line 18: 9-element tuple
type PositionAccumulator = (f64, i64, i64, i64, i64, i64, String, String, String);
```

Code that uses these accesses fields by numeric index:

```rust
entry.0 += qty;          // quantity? cost? who knows
entry.1 += cost;         // total_cost? total_proceeds?
entry.8 = date.clone();  // first_date? last_date?
```

This is error-prone and unreadable. A swap of two `i64` fields in
the accumulator would silently corrupt financial calculations with no
compiler error.

### Fix

Replace with named structs:

```rust
struct PositionAccumulator {
    quantity: f64,
    total_cost: i64,
    total_proceeds: i64,
    total_fees: i64,
    total_taxes: i64,
    total_dividends: i64,
    currency: String,
    first_date: String,
    last_date: String,
}
```

Field access becomes `entry.total_cost += cost`, which is
self-documenting and catches field-swap bugs at compile time.

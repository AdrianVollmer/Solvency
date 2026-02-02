## Negative sub-dollar amounts lose their sign in display functions

**Priority: Correctness (Medium)**

Several `*_display()` methods in `src/models/trading.rs` format cents
by splitting into `dollars` and `cents`:

```rust
// src/models/trading.rs:340-343
pub fn total_cost_display(&self) -> String {
    let dollars = self.total_cost_cents / 100;
    let cents = self.total_cost_cents.abs() % 100;
    format!("{}.{:02}", dollars, cents)
}
```

When `total_cost_cents` is between -99 and -1 (e.g. -50):
- `dollars = -50 / 100 = 0` (truncation toward zero)
- `cents = 50 % 100 = 50`
- Output: `"0.50"` -- **the negative sign is lost**

The correct output should be `"-0.50"`.

### Affected functions

- `ClosedPosition::total_cost_display` (line 340)
- `ClosedPosition::total_proceeds_display` (line 354)
- `Position::current_value_display` (line 272)
- `TradingActivity::fee_display` (line 114) -- also broken for
  negative fees: `-150` produces `"-1.-50"` because the remainder
  is negative without `.abs()`.

### Fix

Use the pattern from `src/filters.rs` instead: compute `abs_cents`,
then prepend the sign explicitly:

```rust
pub fn total_cost_display(&self) -> String {
    let abs = self.total_cost_cents.abs();
    let sign = if self.total_cost_cents < 0 { "-" } else { "" };
    format!("{}{}.{:02}", sign, abs / 100, abs % 100)
}
```

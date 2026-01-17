# DRY: Duplicate Filter Params Structures

## Priority
Low (Maintainability)

## Location
- `src/handlers/expenses.rs:85-148`
- `src/handlers/trading_activities.rs:83-140`

## Description
`ExpenseFilterParams` and `TradingActivityFilterParams` have very similar
structures and nearly identical methods:

Both have:
- `from_date`, `to_date`, `page`, `preset` fields
- `resolve_date_range()` method with identical logic
- `base_query_string()` method with similar logic

```rust
// ExpenseFilterParams
pub fn resolve_date_range(&self) -> DateRange {
    if let Some(preset_str) = &self.preset {
        preset_str.parse::<DatePreset>()
            .map(DateRange::from_preset)
            .unwrap_or_default()
    } else if let (Some(from), Some(to)) = (&self.from_date, &self.to_date) {
        // ...
    }
}

// TradingActivityFilterParams - nearly identical
pub fn resolve_date_range(&self) -> DateRange {
    if let Some(preset_str) = &self.preset {
        preset_str.parse::<DatePreset>()
            .map(DateRange::from_preset)
            .unwrap_or_default()
    } else if let (Some(from), Some(to)) = (&self.from_date, &self.to_date) {
        // ...
    }
}
```

## Recommendation
Extract common date filtering logic into a trait or base struct:

```rust
pub trait DateFilterable {
    fn from_date(&self) -> Option<&String>;
    fn to_date(&self) -> Option<&String>;
    fn preset(&self) -> Option<&String>;

    fn resolve_date_range(&self) -> DateRange {
        // shared implementation
    }
}
```

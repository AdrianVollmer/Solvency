# Tests: CSV Parser Missing Edge Case Coverage

## Priority
Medium (Tests)

## Location
`src/services/csv_parser.rs:220-244`

## Description
The CSV parser tests cover only basic happy paths:

```rust
#[test]
fn test_parse_simple_csv() {
    let csv = b"date,amount,description\n2024-01-15,50.00,Groceries\n2024-01-16,25.50,Coffee";
    let result = parse_csv(csv).unwrap();
    assert_eq!(result.expenses.len(), 2);
}

#[test]
fn test_clean_amount() {
    assert_eq!(clean_amount("$50.00"), "50.00");
    // ...
}
```

Missing test coverage for:
- Malformed CSV (missing columns, extra columns, inconsistent row lengths)
- Special characters in descriptions (quotes, commas, newlines)
- Unicode characters and different encodings
- Very large files (memory and performance)
- Empty files and files with only headers
- Various date formats
- Negative amounts in different formats
- European number formats (1.234,56 vs 1,234.56)

## Recommendation
Add comprehensive test cases:

```rust
#[test]
fn test_parse_csv_with_quoted_fields() {
    let csv = b"date,amount,description\n2024-01-15,50.00,\"Coffee, tea, and snacks\"";
    // ...
}

#[test]
fn test_parse_csv_missing_required_column() {
    let csv = b"date,description\n2024-01-15,Groceries";
    let result = parse_csv(csv);
    assert!(result.is_err());
}

#[test]
fn test_parse_european_amounts() {
    let csv = b"date,amount,description\n2024-01-15,\"1.234,56\",Test";
    let result = parse_csv(csv).unwrap();
    assert_eq!(result.expenses[0].amount, "1234.56");
}
```

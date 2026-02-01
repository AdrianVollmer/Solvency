## Pervasive code duplication across queries and models

**Priority: Maintainability (High)**

### Duplicated `currency_symbol()` function

Identical implementations exist in two model files:
- `src/models/transaction.rs:164-179`
- `src/models/trading.rs:592-607`

A third copy lives in `src/filters.rs:233-260`.

### Duplicated row-to-struct mapping

The categories module maps `rusqlite::Row` to `Category` in **6
separate places** (`src/db/queries/categories.rs` lines 13, 50, 77,
147, 189, 218). The accounts module shows the correct pattern with a
`row_to_account` helper.

Similarly:
- `TransactionWithRelations` mapping is duplicated between
  `list_transactions` and `get_transaction`
  (`src/db/queries/transactions.rs` lines 103 and 244)
- `TradingActivity` mapping is duplicated 3 times
  (`src/db/queries/trading.rs` lines 79, 152, 544)

### Duplicated import session infrastructure

Transaction import (`src/db/queries/import.rs`, 302 lines) and
trading import (`src/db/queries/trading.rs`, ~230 lines) have
nearly identical `create_session`, `get_session`,
`update_session_status`, `delete_session`, etc. functions.

### Duplicated `ParsedTransaction` fallback

`src/db/queries/import.rs` has the same 15-field fallback
`ParsedTransaction` construction copy-pasted at lines 157 and 216.
This should be a `Default` impl.

### Duplicated subtree collection

`src/handlers/api.rs:14` (`collect_subtree_ids`) and
`src/handlers/spending.rs:260` (`collect_descendant_ids`) compute
the same thing with different algorithms.

### Fix

- Move `currency_symbol` to a single shared location.
- Add `row_to_*` helpers for categories, transactions, trading
  activities (like accounts already has).
- Extract a generic import session module parameterized by row type.
- Derive `Default` for `ParsedTransaction`.
- Unify the subtree collection functions.

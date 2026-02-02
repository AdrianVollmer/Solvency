## Multi-step database mutations lack transaction wrapping

**Priority: Correctness (High)**

No `BEGIN`/`COMMIT` (or `rusqlite::Transaction`) is used anywhere in
the codebase. Every `conn.execute()` auto-commits individually. This
means a crash or error partway through a multi-step operation leaves
the database inconsistent.

### Critical cases

**Trading activity create/update** (`src/handlers/trading_activities.rs:497-577`):
Creates an activity, then applies split adjustments in a separate call.
A failure after the INSERT but before the split adjustment leaves an
un-adjusted activity.

**Transaction tag update** (`src/db/queries/transactions.rs:330-377`):
`UPDATE` the transaction, `DELETE` all tags, then `INSERT` new tags.
A crash after the DELETE loses all tag associations.

**Bulk import** (`src/handlers/import.rs:563-662`):
Hundreds of individual INSERTs without a transaction wrapper. Each is a
separate WAL commit, which is also much slower than a batched
transaction.

**delete_all_transactions** (`src/db/queries/transactions.rs:400-405`):
Deletes from `transaction_tags` first, then `transactions`. Crash between
the two leaves orphaned transactions with no tags.

### Fix

Use `let tx = conn.transaction()?;` ... `tx.commit()?;` for any
operation involving multiple statements. This fixes both consistency
and performance (bulk inserts wrapped in a single transaction are
orders of magnitude faster in SQLite).

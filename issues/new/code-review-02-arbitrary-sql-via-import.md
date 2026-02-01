## Database import executes arbitrary user-supplied SQL

**Priority: Security (Critical)**

Two paths in `src/handlers/settings.rs` execute SQL from uploaded files.

### Legacy SQL import (line 198-203)

If the uploaded file doesn't start with SQLite magic bytes, it is
treated as raw SQL and fed directly to `conn.execute_batch()`:

```rust
let sql_content = String::from_utf8(file_bytes)?;
conn.execute_batch("PRAGMA foreign_keys = OFF")?;
conn.execute_batch(&sql_content);  // arbitrary SQL
```

An authenticated user can execute any SQL: DROP TABLE, ATTACH
DATABASE to read/write arbitrary files on disk, etc.

### SQLite binary import (restore_from_db_file)

The code reads `sql` from the uploaded file's `sqlite_master` and
executes it via `execute_batch`:

```rust
for (_, create_sql) in &tables {
    conn.execute_batch(create_sql)?;     // from uploaded file
}
for obj_sql in &extra_objects {
    conn.execute_batch(obj_sql)?;        // triggers, indexes from uploaded file
}
```

A crafted `.db` file can embed malicious triggers or SQL in its
schema.

### Fix

- Remove the legacy SQL import path entirely (`.db` import already
  exists as the replacement).
- For `.db` import, validate table names against an allowlist of
  known schema tables. Do not execute raw `create_sql` from the
  uploaded file -- instead, use the application's own migration
  schema and only `INSERT ... SELECT` data from known tables.

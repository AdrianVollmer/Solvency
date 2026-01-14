-- Import session management for multi-step CSV import wizard

CREATE TABLE import_sessions (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'parsing',
    total_rows INTEGER NOT NULL DEFAULT 0,
    processed_rows INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    errors TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE import_rows (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE,
    row_index INTEGER NOT NULL,
    data TEXT NOT NULL,
    category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    error TEXT,
    UNIQUE(session_id, row_index)
);

CREATE INDEX idx_import_rows_session ON import_rows(session_id);
CREATE INDEX idx_import_rows_status ON import_rows(session_id, status);

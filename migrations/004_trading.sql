-- Trading activities and import management

CREATE TABLE trading_activities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,
    symbol TEXT NOT NULL,
    quantity REAL,
    activity_type TEXT NOT NULL,
    unit_price_cents INTEGER,
    currency TEXT NOT NULL DEFAULT 'USD',
    fee_cents INTEGER NOT NULL DEFAULT 0,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_trading_activities_date ON trading_activities(date DESC);
CREATE INDEX idx_trading_activities_symbol ON trading_activities(symbol);
CREATE INDEX idx_trading_activities_type ON trading_activities(activity_type);

-- Import session management for trading CSV import wizard

CREATE TABLE trading_import_sessions (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'parsing',
    total_rows INTEGER NOT NULL DEFAULT 0,
    processed_rows INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    errors TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE trading_import_rows (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES trading_import_sessions(id) ON DELETE CASCADE,
    row_index INTEGER NOT NULL,
    data TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    error TEXT,
    UNIQUE(session_id, row_index)
);

CREATE INDEX idx_trading_import_rows_session ON trading_import_rows(session_id);
CREATE INDEX idx_trading_import_rows_status ON trading_import_rows(session_id, status);

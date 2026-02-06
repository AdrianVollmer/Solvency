-- AI Categorization Sessions
-- Tracks background AI categorization jobs

CREATE TABLE ai_categorization_sessions (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'pending',
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    total_transactions INTEGER NOT NULL DEFAULT 0,
    processed_transactions INTEGER NOT NULL DEFAULT 0,
    categorized_count INTEGER NOT NULL DEFAULT 0,
    skipped_count INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    errors TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_ai_categorization_sessions_status ON ai_categorization_sessions(status);
CREATE INDEX idx_ai_categorization_sessions_created_at ON ai_categorization_sessions(created_at DESC);

-- AI Categorization Results
-- Stores individual transaction categorization suggestions

CREATE TABLE ai_categorization_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES ai_categorization_sessions(id) ON DELETE CASCADE,
    transaction_id INTEGER NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    original_category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    suggested_category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    confidence REAL,
    ai_reasoning TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(session_id, transaction_id)
);

CREATE INDEX idx_ai_categorization_results_session ON ai_categorization_results(session_id);
CREATE INDEX idx_ai_categorization_results_status ON ai_categorization_results(status);

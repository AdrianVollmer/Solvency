-- API logs for tracking external API calls (Yahoo Finance, etc.)

CREATE TABLE api_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    api_name TEXT NOT NULL DEFAULT 'yahoo_finance',
    action TEXT NOT NULL,
    symbol TEXT,
    request_params TEXT NOT NULL,
    status TEXT NOT NULL,
    response_summary TEXT,
    response_details TEXT,
    duration_ms INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_api_logs_created_at ON api_logs(created_at DESC);
CREATE INDEX idx_api_logs_status ON api_logs(status);
CREATE INDEX idx_api_logs_symbol ON api_logs(symbol);

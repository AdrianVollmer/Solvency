-- Symbol metadata cache (fetched from Yahoo Finance)

CREATE TABLE symbol_metadata (
    symbol TEXT PRIMARY KEY,
    short_name TEXT,
    long_name TEXT,
    exchange TEXT,
    quote_type TEXT,
    fetched_at TEXT NOT NULL DEFAULT (datetime('now'))
);

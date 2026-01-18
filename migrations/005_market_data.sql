-- Market data storage for stock prices

CREATE TABLE market_data (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    date TEXT NOT NULL,
    close_price_cents INTEGER NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(symbol, date)
);

CREATE INDEX idx_market_data_symbol ON market_data(symbol);
CREATE INDEX idx_market_data_date ON market_data(date);
CREATE INDEX idx_market_data_symbol_date ON market_data(symbol, date DESC);

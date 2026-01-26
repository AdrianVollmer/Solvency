-- Accounts for tracking which account transactions and trading activities belong to

CREATE TABLE accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    account_type TEXT NOT NULL CHECK (account_type IN ('Cash', 'Securities')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_accounts_type ON accounts(account_type);

-- Add optional account_id to transactions (only Cash accounts)
ALTER TABLE transactions ADD COLUMN account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL;

CREATE INDEX idx_transactions_account ON transactions(account_id);

-- Add optional account_id to trading_activities (only Securities accounts)
ALTER TABLE trading_activities ADD COLUMN account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL;

CREATE INDEX idx_trading_activities_account ON trading_activities(account_id);

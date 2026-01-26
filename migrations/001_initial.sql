-- MoneyMapper Initial Schema
-- Categories with hierarchical support (adjacency list)
CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    parent_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    color TEXT NOT NULL DEFAULT '#6b7280',
    icon TEXT NOT NULL DEFAULT 'folder',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(name, parent_id)
);

CREATE INDEX IF NOT EXISTS idx_categories_parent ON categories(parent_id);

-- Tags table
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL DEFAULT '#6b7280',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Transactions table
CREATE TABLE IF NOT EXISTS transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,
    amount_cents INTEGER NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    description TEXT NOT NULL,
    category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_category ON transactions(category_id);

-- Many-to-many: Transactions to Tags
CREATE TABLE IF NOT EXISTS transaction_tags (
    transaction_id INTEGER NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (transaction_id, tag_id)
);

CREATE INDEX IF NOT EXISTS idx_transaction_tags_tag ON transaction_tags(tag_id);

-- App settings (key-value store)
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Insert default settings
INSERT OR IGNORE INTO settings (key, value) VALUES
    ('theme', 'system'),
    ('currency', 'USD'),
    ('date_format', 'YYYY-MM-DD'),
    ('page_size', '25'),
    ('locale', 'en-US');

-- Insert default categories
INSERT OR IGNORE INTO categories (id, name, color, icon) VALUES
    (1, 'Food & Dining', '#ef4444', 'utensils'),
    (2, 'Transportation', '#3b82f6', 'car'),
    (3, 'Housing', '#8b5cf6', 'home'),
    (4, 'Utilities', '#f59e0b', 'zap'),
    (5, 'Entertainment', '#ec4899', 'film'),
    (6, 'Shopping', '#10b981', 'shopping-bag'),
    (7, 'Healthcare', '#06b6d4', 'heart'),
    (8, 'Other', '#6b7280', 'more-horizontal');

-- Subcategories for Food & Dining
INSERT OR IGNORE INTO categories (name, parent_id, color, icon) VALUES
    ('Groceries', 1, '#ef4444', 'shopping-cart'),
    ('Restaurants', 1, '#ef4444', 'utensils'),
    ('Coffee & Snacks', 1, '#ef4444', 'coffee');

-- Subcategories for Transportation
INSERT OR IGNORE INTO categories (name, parent_id, color, icon) VALUES
    ('Gas', 2, '#3b82f6', 'fuel'),
    ('Public Transit', 2, '#3b82f6', 'train'),
    ('Parking', 2, '#3b82f6', 'parking');

-- Subcategories for Housing
INSERT OR IGNORE INTO categories (name, parent_id, color, icon) VALUES
    ('Rent/Mortgage', 3, '#8b5cf6', 'home'),
    ('Maintenance', 3, '#8b5cf6', 'wrench'),
    ('Insurance', 3, '#8b5cf6', 'shield');

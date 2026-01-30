-- Solvency Initial Schema
-- Categories with hierarchical support (adjacency list)
CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    parent_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    color TEXT NOT NULL DEFAULT '#6b7280',
    icon TEXT NOT NULL DEFAULT 'folder',
    built_in INTEGER NOT NULL DEFAULT 0,
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

-- Insert built-in root categories
INSERT OR IGNORE INTO categories (id, name, color, icon, built_in) VALUES
    (1, 'Expenses',  '#ef4444', 'trending-down',    1),
    (2, 'Income',    '#10b981', 'trending-up',       1),
    (3, 'Transfers', '#3b82f6', 'arrow-left-right',  1);

-- Default expense categories (children of Expenses)
INSERT OR IGNORE INTO categories (id, name, parent_id, color, icon) VALUES
    (4,  'Food & Dining',  1, '#ef4444', 'utensils'),
    (5,  'Transportation', 1, '#3b82f6', 'car'),
    (6,  'Housing',        1, '#8b5cf6', 'home'),
    (7,  'Utilities',      1, '#f59e0b', 'zap'),
    (8,  'Entertainment',  1, '#ec4899', 'film'),
    (9,  'Shopping',       1, '#10b981', 'shopping-bag'),
    (10, 'Healthcare',     1, '#06b6d4', 'heart'),
    (11, 'Other',          1, '#6b7280', 'more-horizontal');

-- Subcategories for Food & Dining
INSERT OR IGNORE INTO categories (name, parent_id, color, icon) VALUES
    ('Groceries',      4, '#ef4444', 'shopping-cart'),
    ('Restaurants',    4, '#ef4444', 'utensils'),
    ('Coffee & Snacks', 4, '#ef4444', 'coffee');

-- Subcategories for Transportation
INSERT OR IGNORE INTO categories (name, parent_id, color, icon) VALUES
    ('Gas',            5, '#3b82f6', 'fuel'),
    ('Public Transit', 5, '#3b82f6', 'train'),
    ('Parking',        5, '#3b82f6', 'parking');

-- Subcategories for Housing
INSERT OR IGNORE INTO categories (name, parent_id, color, icon) VALUES
    ('Rent/Mortgage',  6, '#8b5cf6', 'home'),
    ('Maintenance',    6, '#8b5cf6', 'wrench'),
    ('Insurance',      6, '#8b5cf6', 'shield');

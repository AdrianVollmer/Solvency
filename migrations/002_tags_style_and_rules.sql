-- Add style field to tags (solid, outline, striped)
ALTER TABLE tags ADD COLUMN style TEXT NOT NULL DEFAULT 'solid';

-- Rules table for automatic categorization
CREATE TABLE IF NOT EXISTS rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    pattern TEXT NOT NULL,
    action_type TEXT NOT NULL,
    action_value TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_rules_action_type ON rules(action_type);

-- Track split adjustments applied to trading activities.
-- When a SPLIT activity is created, prior BUY/SELL quantities and unit prices
-- are adjusted by the split ratio. The original values are stored here so the
-- adjustment can be reversed if the split is deleted or edited.

CREATE TABLE IF NOT EXISTS trading_split_adjustments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    split_activity_id INTEGER NOT NULL REFERENCES trading_activities(id) ON DELETE CASCADE,
    target_activity_id INTEGER NOT NULL REFERENCES trading_activities(id) ON DELETE CASCADE,
    original_quantity REAL NOT NULL,
    original_unit_price_cents INTEGER,
    split_ratio REAL NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(split_activity_id, target_activity_id)
);

CREATE INDEX IF NOT EXISTS idx_split_adjustments_split
    ON trading_split_adjustments(split_activity_id);

CREATE INDEX IF NOT EXISTS idx_split_adjustments_target
    ON trading_split_adjustments(target_activity_id);

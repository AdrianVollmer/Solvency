-- Add detailed transaction fields for bank import compatibility
-- Supports SEPA transaction format (German banks, etc.)

ALTER TABLE expenses ADD COLUMN value_date TEXT;
ALTER TABLE expenses ADD COLUMN payer TEXT;
ALTER TABLE expenses ADD COLUMN payee TEXT;
ALTER TABLE expenses ADD COLUMN reference TEXT;
ALTER TABLE expenses ADD COLUMN transaction_type TEXT;
ALTER TABLE expenses ADD COLUMN counterparty_iban TEXT;
ALTER TABLE expenses ADD COLUMN creditor_id TEXT;
ALTER TABLE expenses ADD COLUMN mandate_reference TEXT;
ALTER TABLE expenses ADD COLUMN customer_reference TEXT;

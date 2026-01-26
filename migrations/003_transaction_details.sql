-- Add detailed transaction fields for bank import compatibility
-- Supports SEPA transaction format (German banks, etc.)

ALTER TABLE transactions ADD COLUMN value_date TEXT;
ALTER TABLE transactions ADD COLUMN payer TEXT;
ALTER TABLE transactions ADD COLUMN payee TEXT;
ALTER TABLE transactions ADD COLUMN reference TEXT;
ALTER TABLE transactions ADD COLUMN transaction_type TEXT;
ALTER TABLE transactions ADD COLUMN counterparty_iban TEXT;
ALTER TABLE transactions ADD COLUMN creditor_id TEXT;
ALTER TABLE transactions ADD COLUMN mandate_reference TEXT;
ALTER TABLE transactions ADD COLUMN customer_reference TEXT;

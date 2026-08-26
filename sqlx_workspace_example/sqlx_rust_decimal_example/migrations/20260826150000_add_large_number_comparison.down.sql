DROP INDEX IF EXISTS financial_data_amount_idx;

ALTER TABLE financial_data
  DROP CONSTRAINT IF EXISTS financial_data_label_key,
  DROP COLUMN IF EXISTS label;

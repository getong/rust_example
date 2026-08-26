ALTER TABLE financial_data
  ADD COLUMN label TEXT;

UPDATE financial_data
SET label = 'legacy_' || id
WHERE label IS NULL;

ALTER TABLE financial_data
  ALTER COLUMN label SET NOT NULL,
  ADD CONSTRAINT financial_data_label_key UNIQUE (label);

CREATE INDEX financial_data_amount_idx
  ON financial_data (amount);

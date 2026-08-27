CREATE TABLE IF NOT EXISTS financial_data (
  id SERIAL PRIMARY KEY,
  amount NUMERIC NOT NULL,
  CONSTRAINT financial_data_amount_is_u256 CHECK (
    scale(amount) = 0
    AND amount >= 0
    AND amount <= 115792089237316195423570985008687907853269984665640564039457584007913129639935
  )
);

CREATE INDEX IF NOT EXISTS financial_data_amount_idx
  ON financial_data (amount);

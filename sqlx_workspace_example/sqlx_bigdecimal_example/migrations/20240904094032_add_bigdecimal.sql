-- Add migration script here
CREATE TABLE IF NOT EXISTS financial_data (
  id SERIAL PRIMARY KEY,
  amount NUMERIC NOT NULL
);

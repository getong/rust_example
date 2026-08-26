CREATE TABLE u256_values (
  id BIGSERIAL PRIMARY KEY,
  label TEXT NOT NULL UNIQUE,
  amount BYTEA NOT NULL,
  CONSTRAINT u256_values_amount_is_32_bytes
    CHECK (octet_length(amount) = 32)
);

CREATE INDEX u256_values_amount_idx
  ON u256_values (amount);

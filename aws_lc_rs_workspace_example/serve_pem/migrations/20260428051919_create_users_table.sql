CREATE TABLE users (
	id BIGSERIAL PRIMARY KEY,
	client_public_key TEXT NOT NULL,
	client_public_key_sha256 CHAR(64) NOT NULL UNIQUE,
	password_hash TEXT NOT NULL,
	created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_created_at ON users (created_at DESC);

-- Add up migration script here
CREATE TABLE IF NOT EXISTS daily_data (
       id SERIAL PRIMARY KEY,
       value TEXT NOT NULL,
       created_at TIMESTAMP NOT NULL,
       updated_at TIMESTAMP NOT NULL
)

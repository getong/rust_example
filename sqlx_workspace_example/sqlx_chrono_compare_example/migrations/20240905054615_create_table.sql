-- Add migration script here
CREATE TABLE IF NOT EXISTS my_data (
       id SERIAL PRIMARY KEY,
       value TEXT NOT NULL,
       created_at TIMESTAMP NOT NULL,
       updated_at TIMESTAMP NOT NULL
)

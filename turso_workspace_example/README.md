# Turso workspace examples

This workspace pins `turso` to `0.7.0-pre.5` and contains three small binary crates.

## Projects

- `in_memory_users`: creates an in-memory database, inserts users, and prints them.
- `file_posts`: creates or opens `my-database.db` and inserts a blog post.
- `remote_sync_notes`: creates a local synced database, writes a note, pushes, and pulls.

## Run

```sh
cargo run -p in_memory_users
cargo run -p file_posts
```

For the sync example, set a real Turso remote URL and token:

```sh
TURSO_REMOTE_URL="libsql://your-database.turso.io" \
TURSO_AUTH_TOKEN="your-token" \
cargo run -p remote_sync_notes
```

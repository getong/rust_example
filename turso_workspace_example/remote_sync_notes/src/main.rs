use std::env;

use turso::sync::Builder;

#[tokio::main]
async fn main() -> turso::Result<()> {
  let remote_url =
    env::var("TURSO_REMOTE_URL").unwrap_or_else(|_| "libsql://your-database.turso.io".into());
  let auth_token = env::var("TURSO_AUTH_TOKEN").unwrap_or_else(|_| "your-token".into());

  let db = Builder::new_remote("local.db")
    .with_remote_url(remote_url)
    .with_auth_token(auth_token)
    .build()
    .await?;

  let conn = db.connect().await?;

  conn
    .execute(
      "CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, content TEXT)",
      (),
    )
    .await?;

  conn
    .execute(
      "INSERT INTO notes (content) VALUES (?1)",
      ["My first synced note"],
    )
    .await?;

  db.push().await?;
  db.pull().await?;

  Ok(())
}

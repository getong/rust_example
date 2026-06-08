use turso::Builder;

#[tokio::main]
async fn main() -> turso::Result<()> {
  let db = Builder::new_local("my-database.db").build().await?;
  let conn = db.connect()?;

  conn
    .execute(
      r#"CREATE TABLE IF NOT EXISTS posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            content TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )"#,
      (),
    )
    .await?;

  let rows_affected = conn
    .execute(
      "INSERT INTO posts (title, content) VALUES (?1, ?2)",
      ["Hello World", "This is my first blog post!"],
    )
    .await?;

  println!("Inserted {rows_affected} rows");

  Ok(())
}

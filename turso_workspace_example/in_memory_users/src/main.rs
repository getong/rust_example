use turso::Builder;

#[tokio::main]
async fn main() -> turso::Result<()> {
  let db = Builder::new_local(":memory:").build().await?;
  let conn = db.connect()?;

  conn
    .execute(
      "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)",
      (),
    )
    .await?;

  conn
    .execute(
      "INSERT INTO users (name, email) VALUES (?1, ?2)",
      ["Alice", "alice@example.com"],
    )
    .await?;

  conn
    .execute(
      "INSERT INTO users (name, email) VALUES (?1, ?2)",
      ["Bob", "bob@example.com"],
    )
    .await?;

  let mut rows = conn.query("SELECT id, name, email FROM users", ()).await?;

  while let Some(row) = rows.next().await? {
    let id = row.get_value(0)?;
    let name = row.get_value(1)?;
    let email = row.get_value(2)?;

    println!(
      "User: {} - {} ({})",
      id.as_integer().copied().unwrap_or_default(),
      name.as_text().map(String::as_str).unwrap_or_default(),
      email.as_text().map(String::as_str).unwrap_or_default(),
    );
  }

  Ok(())
}

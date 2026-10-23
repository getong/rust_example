use stoolap::Database;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let db = Database::open_in_memory()?;

  db.execute(
    "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT UNIQUE,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
    (),
  )?;

  db.execute(
    "INSERT INTO users (id, name, email) VALUES ($1, $2, $3)",
    (1, "Alice", "alice@example.com"),
  )?;

  for row in db.query("SELECT id, name, email FROM users WHERE id = $1", (1,))? {
    let row = row?;
    println!(
      "id={} name={} email={}",
      row.get::<i64>(0)?,
      row.get::<String>(1)?,
      row.get::<String>(2)?
    );
  }

  Ok(())
}

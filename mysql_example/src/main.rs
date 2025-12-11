use std::env;

use mysql::{params, prelude::*, Opts, Pool};

#[derive(Debug, PartialEq, Eq)]
struct Row {
  id: i32,
  name: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Connect to TiDB (MySQL compatible). Override with TIDB_URL if needed.
  // Default matches the container started by tidb/start_tidb.sh.
  let url =
    env::var("TIDB_URL").unwrap_or_else(|_| "mysql://root@127.0.0.1:4000/test_db".to_string());
  let opts = Opts::from_url(&url)?;
  let pool = Pool::new(opts)?;
  let mut conn = pool.get_conn()?;

  // Ensure table exists (compatible with init SQL).
  conn.query_drop(
    r"CREATE TABLE IF NOT EXISTS my_table (
            id INT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(255) NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
  )?;

  // Insert a couple of rows.
  let names = vec!["Rust via TiDB", "Hello TiDB"];
  conn.exec_batch(
    "INSERT INTO my_table (name) VALUES (:name)",
    names.iter().map(|name| params! { "name" => name }),
  )?;

  // Fetch the last few rows and print them.
  let rows: Vec<Row> = conn.query_map(
    "SELECT id, name FROM my_table ORDER BY id DESC LIMIT 5",
    |(id, name)| Row { id, name },
  )?;

  println!("Fetched rows:");
  for row in rows {
    println!("  id={}, name={}", row.id, row.name);
  }

  Ok(())
}

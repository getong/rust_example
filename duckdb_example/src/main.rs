use duckdb::{Connection, Result, params};

struct Duck {
  id: i32,
  name: String,
}

fn main() -> Result<()> {
  let conn = Connection::open_in_memory()?;

  conn.execute(
    "CREATE TABLE ducks (id INTEGER PRIMARY KEY, name TEXT)",
    [], // empty list of parameters
  )?;

  conn.execute_batch(
    r#"
        INSERT INTO ducks (id, name) VALUES (1, 'Donald Duck');
        INSERT INTO ducks (id, name) VALUES (2, 'Scrooge McDuck');
        "#,
  )?;

  conn.execute(
    "INSERT INTO ducks (id, name) VALUES (?, ?)",
    params![3, "Darkwing Duck"],
  )?;

  let ducks = conn
    .prepare("FROM ducks")?
    .query_map([], |row| {
      Ok(Duck {
        id: row.get(0)?,
        name: row.get(1)?,
      })
    })?
    .collect::<Result<Vec<_>>>()?;

  for duck in ducks {
    println!("{}) {}", duck.id, duck.name);
  }

  Ok(())
}

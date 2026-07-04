use std::borrow::Cow;

use fjall::{Database, Guard, Keyspace, KeyspaceCreateOptions, PersistMode};

const ITEMS_KEYSPACE: &str = "items";
const AUDIT_KEYSPACE: &str = "audit_log";

fn main() -> fjall::Result<()> {
  let db_path = std::env::temp_dir().join("fjall-example-demo");

  // A database may contain multiple keyspaces. For most applications, keep one
  // Database handle alive and create separate keyspaces for separate data sets.
  let db = Database::builder(&db_path).open()?;
  let items = db.keyspace(ITEMS_KEYSPACE, KeyspaceCreateOptions::default)?;
  let audit_log = db.keyspace(AUDIT_KEYSPACE, KeyspaceCreateOptions::default)?;

  // Keep this demo deterministic when it is run more than once.
  items.clear()?;
  audit_log.clear()?;

  seed_data(&db, &items, &audit_log)?;

  println!("database path: {}", db_path.display());
  println!("items stored: {}", items.len()?);
  println!("audit events stored: {}", audit_log.len()?);

  print_lookup(&items, "user:100:name")?;
  print_lookup(&items, "user:999:name")?;

  print_scan("prefix user:100:", items.prefix("user:100:"))?;
  print_scan(
    "range user:100: through user:200:",
    items.range("user:100:" ..= "user:200:~"),
  )?;
  print_scan("reverse prefix user:", items.prefix("user:").rev())?;

  items.remove("user:100:city")?;
  println!("removed user:100:city");
  print_lookup(&items, "user:100:city")?;

  print_scan("audit log:", audit_log.iter())?;

  // Sync the journal to disk to make sure data is durable.
  db.persist(PersistMode::SyncAll)
}

fn seed_data(db: &Database, items: &Keyspace, audit_log: &Keyspace) -> fjall::Result<()> {
  let mut batch = db.batch();

  for (key, value) in [
    ("user:100:name", "Alice"),
    ("user:100:city", "Shanghai"),
    ("user:200:name", "Bob"),
    ("user:200:city", "Beijing"),
    ("user:300:name", "Carol"),
    ("order:1000:status", "paid"),
  ] {
    batch.insert(items, key, value);
  }

  batch.insert(audit_log, "event:0001", "seeded demo records");
  batch.commit()
}

fn print_lookup(items: &Keyspace, key: &str) -> fjall::Result<()> {
  match items.get(key)? {
    Some(value) => println!("get {key}: {}", display_bytes(&value)),
    None => println!("get {key}: <missing>"),
  }

  Ok(())
}

fn print_scan(rows_name: &str, rows: impl IntoIterator<Item = Guard>) -> fjall::Result<()> {
  println!("{rows_name}");

  for guard in rows {
    let (key, value) = guard.into_inner()?;
    println!("  {} => {}", display_bytes(&key), display_bytes(&value));
  }

  Ok(())
}

fn display_bytes(bytes: &[u8]) -> Cow<'_, str> {
  String::from_utf8_lossy(bytes)
}

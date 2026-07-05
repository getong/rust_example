use std::{borrow::Cow, path::Path};

use fjall::{Database, Guard, Keyspace, KeyspaceCreateOptions, PersistMode};

const ITEMS_KEYSPACE: &str = "items";
const AUDIT_KEYSPACE: &str = "audit_log";
const RECOVERY_KEYSPACE: &str = "journal_replay";

fn main() -> fjall::Result<()> {
  let db_dir = tempfile::tempdir()?;
  let db_path = db_dir.path();

  {
    // fjall calls its WAL a journal. Opening the database creates or recovers
    // the active journal file, such as 0.jnl, in the database directory.
    let db = Database::builder(&db_path).open()?;
    print_journal_file_hint(&db_path);

    // A database may contain multiple keyspaces. For most applications, keep one
    // Database handle alive and create separate keyspaces for separate data sets.
    let items = db.keyspace(ITEMS_KEYSPACE, KeyspaceCreateOptions::default)?;
    let audit_log = db.keyspace(AUDIT_KEYSPACE, KeyspaceCreateOptions::default)?;

    // Keep this demo deterministic when it is run more than once. These clear
    // operations are writes too, so they are journaled before the keyspaces are
    // updated.
    items.clear()?;
    audit_log.clear()?;

    seed_data(&db, &items, &audit_log)?;
    print_journal_state(&db, "after seed batch")?;

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
    print_journal_state(&db, "after remove tombstone")?;

    print_scan("audit log:", audit_log.iter())?;

    // By default, each write flushes fjall's journal to the OS buffer
    // (PersistMode::Buffer). SyncAll fsyncs the active journal, making previous
    // writes durable against OS crash or power loss on filesystems where fsync
    // provides that guarantee.
    db.persist(PersistMode::SyncAll)?;
    print_journal_state(&db, "after explicit SyncAll")?;
  }

  // Reopen the database to demonstrate the journal's recovery role. If recent
  // writes had not yet been flushed into LSM table files, fjall can replay the
  // journal and rebuild the in-memory memtables.
  let db = Database::builder(&db_path).open()?;
  let items = db.keyspace(ITEMS_KEYSPACE, KeyspaceCreateOptions::default)?;
  let audit_log = db.keyspace(AUDIT_KEYSPACE, KeyspaceCreateOptions::default)?;

  println!("reopened database from: {}", db_path.display());
  print_lookup(&items, "user:100:name")?;
  print_lookup(&items, "user:100:city")?;
  print_scan("reopened audit log:", audit_log.iter())?;
  print_journal_state(&db, "after reopen")?;

  db.persist(PersistMode::SyncAll)?;

  demonstrate_journal_replay_recovery()
}

fn seed_data(db: &Database, items: &Keyspace, audit_log: &Keyspace) -> fjall::Result<()> {
  let mut batch = db.batch();

  // WriteBatch::commit appends one atomic batch to the journal before the LSM
  // memtables are changed. The batch may touch multiple keyspaces while still
  // being represented by one journaled sequence number.
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

fn print_journal_state(db: &Database, label: &str) -> fjall::Result<()> {
  println!(
    "{label}: journals={}, journal disk bytes={}",
    db.journal_count(),
    db.journal_disk_space()?,
  );

  Ok(())
}

fn print_journal_file_hint(db_path: &Path) {
  println!(
    "active journal path hint: {}",
    db_path.join("0.jnl").display()
  );
}

fn demonstrate_journal_replay_recovery() -> fjall::Result<()> {
  let recovery_dir = tempfile::tempdir()?;
  let recovery_path = recovery_dir.path();

  println!();
  println!("journal replay recovery demo");

  {
    let db = Database::builder(&recovery_path).open()?;
    let events = db.keyspace(RECOVERY_KEYSPACE, KeyspaceCreateOptions::default)?;

    let mut batch = db.batch();
    batch.insert(&events, "batch:0001", "written before close");
    batch.insert(&events, "batch:0002", "also journaled");
    batch.insert(&events, "batch:deleted", "will be removed");
    batch.commit()?;

    events.remove("batch:deleted")?;

    // This demo intentionally does not ask the keyspace/LSM tree to flush a
    // table file. The durable source of truth for the recent batch is the
    // journal. On the next open, Database::recover reads .jnl files and replays
    // these batch records into memtables.
    db.persist(PersistMode::SyncAll)?;
    print_journal_file_hint(&recovery_path);
    print_journal_state(&db, "before drop, journal contains replayable writes")?;
  }

  let recovered_db = Database::builder(&recovery_path).open()?;
  let recovered_events =
    recovered_db.keyspace(RECOVERY_KEYSPACE, KeyspaceCreateOptions::default)?;

  println!(
    "reopened journal replay demo from: {}",
    recovery_path.display()
  );
  print_lookup(&recovered_events, "batch:0001")?;
  print_lookup(&recovered_events, "batch:0002")?;
  print_lookup(&recovered_events, "batch:deleted")?;
  print_scan("replayed journal records:", recovered_events.iter())?;
  print_journal_state(&recovered_db, "after replay recovery")?;

  recovered_db.persist(PersistMode::SyncAll)
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

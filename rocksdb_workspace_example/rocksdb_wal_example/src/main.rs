use std::{
  env,
  error::Error,
  fs,
  io::{self, Write},
  path::Path,
  process,
};

use rocksdb::{DB, Options, WriteBatch, WriteBatchIterator, WriteOptions};

const DB_PATH: &str = "rocksdb_wal_example_db";
const ACCOUNT_1: &[u8] = b"account:1";
const ACCOUNT_2: &[u8] = b"account:2";
const PENDING_ORDER: &[u8] = b"order:pending";

type ExampleResult<T> = Result<T, Box<dyn Error>>;

fn main() -> ExampleResult<()> {
  let command = env::args().nth(1).unwrap_or_else(|| "demo".to_owned());

  match command.as_str() {
    "demo" => run_demo(),
    "crash" => crash_after_synced_wal(),
    "recover" => recover_from_wal(),
    "show-wal" => show_wal_batches(),
    "clean" => clean_database(),
    _ => {
      print_usage();
      Ok(())
    }
  }
}

fn run_demo() -> ExampleResult<()> {
  clean_database()?;

  {
    let db = open_db()?;
    write_with_wal(&db, "normal-demo")?;
    print_wal_files()?;
    print_wal_batches(&db, 0)?;
  }

  println!();
  println!("reopen the database and read data that was persisted through WAL:");
  recover_from_wal()
}

fn crash_after_synced_wal() -> ExampleResult<()> {
  let db = open_db()?;
  write_with_wal(&db, "crash-demo")?;
  print_wal_files()?;

  println!();
  println!("WAL has been flushed and synced. aborting now to simulate a process crash.");
  println!("run `cargo run -p rocksdb_wal_example -- recover` next.");
  io::stdout().flush()?;
  process::abort()
}

fn recover_from_wal() -> ExampleResult<()> {
  let db = open_db()?;

  println!("latest sequence number: {}", db.latest_sequence_number());
  print_value(&db, ACCOUNT_1)?;
  print_value(&db, ACCOUNT_2)?;
  print_value(&db, PENDING_ORDER)?;
  Ok(())
}

fn show_wal_batches() -> ExampleResult<()> {
  let db = open_db()?;
  print_wal_batches(&db, 0)
}

fn open_db() -> ExampleResult<DB> {
  DB::open(&db_options(), DB_PATH).map_err(Into::into)
}

fn db_options() -> Options {
  let mut options = Options::default();
  options.create_if_missing(true);
  // Keep archived WAL files for the demo so `show-wal` can inspect recent batches.
  options.set_wal_ttl_seconds(60 * 60);
  options.set_wal_size_limit_mb(64);
  options
}

fn wal_write_options() -> WriteOptions {
  let mut write_options = WriteOptions::default();
  // WAL is enabled by default. Keep this explicit so the example shows the switch.
  write_options.disable_wal(false);
  // `sync=true` asks RocksDB to fsync the WAL before the write is considered complete.
  write_options.set_sync(true);
  write_options
}

fn write_with_wal(db: &DB, label: &str) -> ExampleResult<()> {
  let before = db.latest_sequence_number();
  let mut batch = WriteBatch::default();
  batch.put(ACCOUNT_1, format!("{label}:balance=100"));
  batch.put(ACCOUNT_2, format!("{label}:balance=250"));
  batch.put(PENDING_ORDER, format!("{label}:order-ready"));

  db.write_opt(batch, &wal_write_options())?;
  // This syncs the live WAL file. Do not call `db.flush()` here; leaving the
  // memtable unflushed demonstrates that recovery can replay the WAL file.
  db.flush_wal(true)?;

  let after = db.latest_sequence_number();
  println!("wrote batch through WAL: sequence {before} -> {after}");
  Ok(())
}

fn print_value(db: &DB, key: &[u8]) -> ExampleResult<()> {
  match db.get(key)? {
    Some(value) => println!(
      "{} => {}",
      display_bytes(key),
      String::from_utf8_lossy(&value)
    ),
    None => println!("{} is missing", display_bytes(key)),
  }
  Ok(())
}

fn print_wal_files() -> ExampleResult<()> {
  println!();
  println!("WAL/log files under `{DB_PATH}`:");

  if !Path::new(DB_PATH).exists() {
    println!("  database directory does not exist yet");
    return Ok(());
  }

  let mut found = false;
  for entry in fs::read_dir(DB_PATH)? {
    let entry = entry?;
    let name = entry.file_name();
    let name = name.to_string_lossy();
    if name.ends_with(".log") || name == "archive" {
      println!("  {name}");
      found = true;
    }
  }

  if !found {
    println!("  no WAL files are visible yet");
  }

  Ok(())
}

fn print_wal_batches(db: &DB, start_sequence: u64) -> ExampleResult<()> {
  println!();
  println!(
    "WAL batches after sequence {start_sequence}; latest sequence is {}:",
    db.latest_sequence_number()
  );

  let mut found = false;
  let mut iterator = db.get_updates_since(start_sequence)?;
  for item in &mut iterator {
    let (sequence, batch) = item?;
    println!("  batch at sequence {sequence}, {} operations", batch.len());
    let mut printer = BatchPrinter;
    batch.iterate(&mut printer);
    found = true;
  }
  iterator.status()?;

  if !found {
    println!("  no retained WAL updates found");
  }

  Ok(())
}

fn clean_database() -> ExampleResult<()> {
  let path = Path::new(DB_PATH);
  if path.exists() {
    if path.read_dir()?.next().is_some() {
      DB::destroy(&db_options(), DB_PATH)?;
    }
    if path.exists() {
      fs::remove_dir_all(path)?;
    }
    println!("removed `{DB_PATH}`");
  }
  Ok(())
}

fn display_bytes(bytes: &[u8]) -> String {
  String::from_utf8_lossy(bytes).into_owned()
}

fn print_usage() {
  println!("usage:");
  println!("  cargo run -p rocksdb_wal_example -- demo");
  println!("  cargo run -p rocksdb_wal_example -- crash");
  println!("  cargo run -p rocksdb_wal_example -- recover");
  println!("  cargo run -p rocksdb_wal_example -- show-wal");
  println!("  cargo run -p rocksdb_wal_example -- clean");
}

struct BatchPrinter;

impl WriteBatchIterator for BatchPrinter {
  fn put(&mut self, key: &[u8], value: &[u8]) {
    println!("    put {} => {}", display_bytes(key), display_bytes(value));
  }

  fn delete(&mut self, key: &[u8]) {
    println!("    delete {}", display_bytes(key));
  }
}

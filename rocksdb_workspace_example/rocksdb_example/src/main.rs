use rocksdb::{ColumnFamilyDescriptor, DB, Direction, IteratorMode, Options, WriteBatch};

fn main() {
  basic_kv_example();
  batch_and_iterator_example();
  column_family_example();
}

fn basic_kv_example() {
  let path = "rocksdb_example_basic";
  let db = DB::open_default(path).expect("open default db");

  db.put(b"my key", b"my value").expect("write value");
  match db.get(b"my key") {
    Ok(Some(value)) => println!("retrieved value {}", String::from_utf8_lossy(&value)),
    Ok(None) => println!("value not found"),
    Err(e) => println!("operational problem encountered: {}", e),
  }
  db.delete(b"my key").expect("delete value");

  DB::destroy(&Options::default(), path).expect("destroy db");
}

fn batch_and_iterator_example() {
  let path = "rocksdb_example_batch";
  let db = DB::open_default(path).expect("open default db");

  // Write a batch of related updates atomically.
  let mut batch = WriteBatch::default();
  batch.put(b"user:1", b"alice");
  batch.put(b"user:2", b"bob");
  batch.put(b"cfg:theme", b"sunrise");
  batch.delete(b"cfg:theme");
  db.write(batch).expect("write batch");

  println!("full scan:");
  for item in db.iterator(IteratorMode::Start) {
    let (key, value) = item.expect("iterator item");
    println!(
      "  {} => {}",
      String::from_utf8_lossy(&key),
      String::from_utf8_lossy(&value)
    );
  }

  println!("prefix scan for user:");
  for item in db.iterator(IteratorMode::From(b"user:", Direction::Forward)) {
    let (key, value) = item.expect("iterator item");
    if !key.starts_with(b"user:") {
      break;
    }
    println!(
      "  {} => {}",
      String::from_utf8_lossy(&key),
      String::from_utf8_lossy(&value)
    );
  }

  DB::destroy(&Options::default(), path).expect("destroy db");
}

fn column_family_example() {
  let path = "rocksdb_example_cf";
  let mut cf_opts = Options::default();
  cf_opts.set_max_write_buffer_number(16);
  let cf = ColumnFamilyDescriptor::new("cf1", cf_opts);

  let mut db_opts = Options::default();
  db_opts.create_missing_column_families(true);
  db_opts.create_if_missing(true);

  {
    let db = DB::open_cf_descriptors(&db_opts, path, vec![cf]).expect("open db with cf1");
    let cf1 = db.cf_handle("cf1").expect("get cf1 handle");
    db.put_cf(cf1, b"cf key", b"cf value")
      .expect("write cf1 value");
    let value = db
      .get_cf(cf1, b"cf key")
      .expect("read cf1 value")
      .expect("cf1 value present");
    println!("cf1 value {}", String::from_utf8_lossy(&value));
  }

  DB::destroy(&db_opts, path).expect("destroy db");
}

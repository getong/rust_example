use rocksdb::{Options, DB};

fn main() {
  // println!("Hello, world!");

  let path = "_path_for_rocksdb_storage_with_cfs";
  // let mut cf_opts = Options::default();
  // cf_opts.set_max_write_buffer_number(16);
  // let cf = ColumnFamilyDescriptor::new("cf1", cf_opts);

  // let mut db_opts = Options::default();
  // db_opts.create_missing_column_families(true);
  // db_opts.create_if_missing(true);
  // {
  //     let db = DB::open_cf_descriptors(&db_opts, path, vec![cf]).unwrap();
  // }

  {
    let db = DB::open_default(path).unwrap();
    db.put(b"my key", b"my value").unwrap();
    match db.get(b"my key") {
      Ok(Some(value)) => println!("retrieved value {}", String::from_utf8(value).unwrap()),
      Ok(None) => println!("value not found"),
      Err(e) => println!("operational problem encountered: {}", e),
    }
    db.delete(b"my key").unwrap();
  }

  // let _ = DB::destroy(&db_opts, path);
  let _ = DB::destroy(&Options::default(), path);
}

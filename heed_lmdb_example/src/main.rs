use heed::{Database, EnvOpenOptions, byteorder, types::*};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let db_path = "my-first-db";
  // LMDB requires the environment directory to exist before opening it.
  std::fs::create_dir_all(db_path)?;
  let env = unsafe { EnvOpenOptions::new().open(db_path)? };

  // We open the default unnamed database
  let mut wtxn = env.write_txn()?;
  let db: Database<Str, U32<byteorder::NativeEndian>> = env.create_database(&mut wtxn, None)?;

  // We open a write transaction
  db.put(&mut wtxn, "seven", &7)?;
  db.put(&mut wtxn, "zero", &0)?;
  db.put(&mut wtxn, "five", &5)?;
  db.put(&mut wtxn, "three", &3)?;
  wtxn.commit()?;

  // We open a read transaction to check if those values are now available
  let rtxn = env.read_txn()?;

  let ret = db.get(&rtxn, "zero")?;
  assert_eq!(ret, Some(0));

  let ret = db.get(&rtxn, "five")?;
  assert_eq!(ret, Some(5));

  Ok(())
}

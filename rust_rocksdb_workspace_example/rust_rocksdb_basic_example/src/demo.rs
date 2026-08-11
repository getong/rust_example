use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use tokio::task::JoinSet;

use crate::{AsyncRocksDb, BatchOperation};

/// Runs a compact tour of the features used most often in service code.
pub async fn run_demo(path: PathBuf) -> Result<()> {
  println!("database path: {}", path.display());

  let db = AsyncRocksDb::open(path, ["users"])
    .await
    .context("failed to open the RocksDB demo database")?;

  println!("\n1. Basic CRUD");
  db.put("session:demo", "active").await?;
  print_value("session:demo", db.get("session:demo").await?);

  println!("\n2. Column families isolate logical data sets");
  db.put("profile:42", "value in default column family")
    .await?;
  db.put_cf("users", "profile:42", "value in users column family")
    .await?;
  print_value("default/profile:42", db.get("profile:42").await?);
  print_value("users/profile:42", db.get_cf("users", "profile:42").await?);

  println!("\n3. WriteBatch applies related changes atomically");
  db.put("cart:demo", "item-42").await?;
  db.write_batch([
    BatchOperation::put("order:1001:state", "pending"),
    BatchOperation::put("order:1001:amount", "12800"),
    BatchOperation::delete("cart:demo"),
  ])
  .await?;

  let values = db
    .multi_get(["order:1001:state", "order:1001:amount", "cart:demo"])
    .await?;
  for (key, value) in ["order:1001:state", "order:1001:amount", "cart:demo"]
    .into_iter()
    .zip(values)
  {
    print_value(key, value);
  }

  println!("\n4. Tokio tasks share one thread-safe DB handle");
  let mut writes = JoinSet::new();
  for worker in 0 .. 8 {
    let db = db.clone();
    writes.spawn(async move {
      let key = format!("event:{worker:02}");
      let value = format!("written by task {worker}");
      db.put(key, value).await
    });
  }
  while let Some(result) = writes.join_next().await {
    result.context("a demo writer task failed")??;
  }

  println!("\n5. Ordered prefix scan");
  for (key, value) in db.scan_prefix("event:", 100).await? {
    println!(
      "{} = {}",
      String::from_utf8_lossy(&key),
      String::from_utf8_lossy(&value)
    );
  }

  println!("\n6. Operational information and durability boundary");
  println!(
    "estimated keys in default column family: {:?}",
    db.estimated_key_count().await?
  );
  println!(
    "latest sequence number: {}",
    db.latest_sequence_number().await?
  );
  db.flush_wal(true).await?;
  println!("WAL synced to durable storage");

  db.delete("session:demo").await?;
  ensure!(
    db.get("session:demo").await?.is_none(),
    "deleted key is still visible"
  );
  println!("\n7. Delete verified: session:demo = <missing>");

  db.close().await?;
  Ok(())
}

fn print_value(key: &str, value: Option<Vec<u8>>) {
  match value {
    Some(value) => println!("{key} = {}", String::from_utf8_lossy(&value)),
    None => println!("{key} = <missing>"),
  }
}

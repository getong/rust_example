use rust_rocksdb_basic_example::{AsyncRocksDb, BatchOperation};
use tempfile::TempDir;
use tokio::task::JoinSet;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crud_and_delete_round_trip() -> Result<(), Box<dyn std::error::Error>> {
  let directory = TempDir::new()?;
  let db = AsyncRocksDb::open(directory.path(), std::iter::empty::<&str>()).await?;

  assert_eq!(db.get("missing").await?, None);
  db.put("name", "rocksdb").await?;
  assert_eq!(db.get("name").await?, Some(b"rocksdb".to_vec()));

  db.delete("name").await?;
  assert_eq!(db.get("name").await?, None);

  db.close().await?;
  Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_scan_multi_get_and_column_family_work() -> Result<(), Box<dyn std::error::Error>> {
  let directory = TempDir::new()?;
  let db = AsyncRocksDb::open(directory.path(), ["users"]).await?;

  db.put("temporary", "remove-me").await?;
  db.write_batch([
    BatchOperation::put("item:02", "second"),
    BatchOperation::put("item:01", "first"),
    BatchOperation::delete("temporary"),
  ])
  .await?;

  let values = db.multi_get(["item:01", "item:02", "temporary"]).await?;
  assert_eq!(
    values,
    vec![Some(b"first".to_vec()), Some(b"second".to_vec()), None]
  );

  let entries = db.scan_prefix("item:", 10).await?;
  assert_eq!(
    entries,
    vec![
      (b"item:01".to_vec(), b"first".to_vec()),
      (b"item:02".to_vec(), b"second".to_vec()),
    ]
  );

  db.put("same-key", "default").await?;
  db.put_cf("users", "same-key", "users").await?;
  assert_eq!(db.get("same-key").await?, Some(b"default".to_vec()));
  assert_eq!(
    db.get_cf("users", "same-key").await?,
    Some(b"users".to_vec())
  );

  db.close().await?;
  Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cloned_handle_supports_concurrent_tokio_tasks() -> Result<(), Box<dyn std::error::Error>> {
  let directory = TempDir::new()?;
  let db = AsyncRocksDb::open(directory.path(), std::iter::empty::<&str>()).await?;
  let mut writes = JoinSet::new();

  for index in 0 .. 32 {
    let db = db.clone();
    writes.spawn(async move {
      db.put(format!("worker:{index:02}"), index.to_string())
        .await
    });
  }

  while let Some(result) = writes.join_next().await {
    result??;
  }

  assert_eq!(db.scan_prefix("worker:", 100).await?.len(), 32);
  db.close().await?;
  Ok(())
}

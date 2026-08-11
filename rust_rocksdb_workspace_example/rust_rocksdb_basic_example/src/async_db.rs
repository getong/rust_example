use std::{num::NonZeroUsize, path::PathBuf, sync::Arc};

use rust_rocksdb::{DB, Direction, IteratorMode, Options, WriteBatch};
use thiserror::Error;
use tokio::{
  sync::Semaphore,
  task::{self, JoinError},
};

const DEFAULT_MAX_BLOCKING_OPERATIONS: usize = 16;

/// A key/value pair copied out of RocksDB and safe to use after the iterator closes.
pub type KeyValue = (Vec<u8>, Vec<u8>);

/// Errors produced by [`AsyncRocksDb`].
#[derive(Debug, Error)]
pub enum AsyncDbError {
  /// The underlying RocksDB operation failed.
  #[error("rocksdb operation failed: {0}")]
  RocksDb(#[from] rust_rocksdb::Error),

  /// Tokio could not complete a blocking task.
  #[error("rocksdb blocking task failed: {0}")]
  BlockingTask(#[from] JoinError),

  /// The requested column family was not opened with the database.
  #[error("column family `{0}` is not open")]
  MissingColumnFamily(String),

  /// The operation limiter was closed while a caller was waiting.
  #[error("rocksdb operation limiter is closed")]
  OperationLimiterClosed,
}

/// One operation in an atomic RocksDB write batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BatchOperation {
  /// Insert or replace one value.
  Put { key: Vec<u8>, value: Vec<u8> },
  /// Delete one key if it exists.
  Delete { key: Vec<u8> },
}

impl BatchOperation {
  /// Creates a batch put operation.
  #[must_use]
  pub fn put(key: impl AsRef<[u8]>, value: impl AsRef<[u8]>) -> Self {
    Self::Put {
      key: key.as_ref().to_vec(),
      value: value.as_ref().to_vec(),
    }
  }

  /// Creates a batch delete operation.
  #[must_use]
  pub fn delete(key: impl AsRef<[u8]>) -> Self {
    Self::Delete {
      key: key.as_ref().to_vec(),
    }
  }
}

/// A cloneable RocksDB handle that keeps blocking calls off Tokio worker threads.
///
/// `rust-rocksdb` exposes a synchronous API. Each method here owns its arguments,
/// acquires a bounded permit, and executes the RocksDB call with
/// [`tokio::task::spawn_blocking`].
#[derive(Clone)]
pub struct AsyncRocksDb {
  inner: Arc<DB>,
  blocking_slots: Arc<Semaphore>,
}

impl AsyncRocksDb {
  /// Opens a database and creates any listed column families that are missing.
  ///
  /// At most 16 RocksDB calls are submitted concurrently by default.
  ///
  /// # Errors
  ///
  /// Returns an error if Tokio cannot run the blocking task or RocksDB cannot
  /// open the database.
  pub async fn open<P, I, S>(path: P, column_families: I) -> Result<Self, AsyncDbError>
  where
    P: Into<PathBuf>,
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    let max_concurrency = NonZeroUsize::new(DEFAULT_MAX_BLOCKING_OPERATIONS)
      .expect("the default RocksDB concurrency limit must be non-zero");
    Self::open_with_max_concurrency(path, column_families, max_concurrency).await
  }

  /// Opens a database with a caller-defined blocking-operation limit.
  ///
  /// # Errors
  ///
  /// Returns an error if Tokio cannot run the blocking task or RocksDB cannot
  /// open the database.
  pub async fn open_with_max_concurrency<P, I, S>(
    path: P,
    column_families: I,
    max_concurrency: NonZeroUsize,
  ) -> Result<Self, AsyncDbError>
  where
    P: Into<PathBuf>,
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    let path = path.into();
    let column_families: Vec<String> = column_families.into_iter().map(Into::into).collect();

    let inner = task::spawn_blocking(move || {
      let mut options = Options::default();
      options.create_if_missing(true);
      options.create_missing_column_families(true);

      if column_families.is_empty() {
        DB::open(&options, path)
      } else {
        DB::open_cf(&options, path, column_families)
      }
    })
    .await??;

    Ok(Self {
      inner: Arc::new(inner),
      blocking_slots: Arc::new(Semaphore::new(max_concurrency.get())),
    })
  }

  /// Inserts or replaces a value in the default column family.
  ///
  /// # Errors
  ///
  /// Returns an error when the limiter, Tokio task, or RocksDB operation fails.
  pub async fn put(
    &self,
    key: impl AsRef<[u8]>,
    value: impl AsRef<[u8]>,
  ) -> Result<(), AsyncDbError> {
    let key = key.as_ref().to_vec();
    let value = value.as_ref().to_vec();
    self
      .execute(move |db| {
        db.put(key, value)?;
        Ok(())
      })
      .await
  }

  /// Gets a copied value from the default column family.
  ///
  /// # Errors
  ///
  /// Returns an error when the limiter, Tokio task, or RocksDB operation fails.
  pub async fn get(&self, key: impl AsRef<[u8]>) -> Result<Option<Vec<u8>>, AsyncDbError> {
    let key = key.as_ref().to_vec();
    self.execute(move |db| Ok(db.get(key)?)).await
  }

  /// Deletes a key from the default column family.
  ///
  /// # Errors
  ///
  /// Returns an error when the limiter, Tokio task, or RocksDB operation fails.
  pub async fn delete(&self, key: impl AsRef<[u8]>) -> Result<(), AsyncDbError> {
    let key = key.as_ref().to_vec();
    self
      .execute(move |db| {
        db.delete(key)?;
        Ok(())
      })
      .await
  }

  /// Reads several keys in one RocksDB call while preserving input order.
  ///
  /// # Errors
  ///
  /// Returns an error when the limiter, Tokio task, or any RocksDB read fails.
  pub async fn multi_get<I, K>(&self, keys: I) -> Result<Vec<Option<Vec<u8>>>, AsyncDbError>
  where
    I: IntoIterator<Item = K>,
    K: AsRef<[u8]>,
  {
    let keys: Vec<Vec<u8>> = keys.into_iter().map(|key| key.as_ref().to_vec()).collect();

    self
      .execute(move |db| {
        let results = db.multi_get(keys);
        let mut values = Vec::with_capacity(results.len());
        for result in results {
          values.push(result?);
        }
        Ok(values)
      })
      .await
  }

  /// Applies all operations atomically to the default column family.
  ///
  /// # Errors
  ///
  /// Returns an error when the limiter, Tokio task, or RocksDB write fails.
  pub async fn write_batch(
    &self,
    operations: impl IntoIterator<Item = BatchOperation>,
  ) -> Result<(), AsyncDbError> {
    let operations: Vec<BatchOperation> = operations.into_iter().collect();
    self
      .execute(move |db| {
        let mut batch = WriteBatch::default();
        for operation in operations {
          match operation {
            BatchOperation::Put { key, value } => batch.put(key, value),
            BatchOperation::Delete { key } => batch.delete(key),
          }
        }
        db.write(&batch)?;
        Ok(())
      })
      .await
  }

  /// Inserts or replaces a value in a named column family.
  ///
  /// # Errors
  ///
  /// Returns an error when the column family is missing or the operation fails.
  pub async fn put_cf(
    &self,
    column_family: impl Into<String>,
    key: impl AsRef<[u8]>,
    value: impl AsRef<[u8]>,
  ) -> Result<(), AsyncDbError> {
    let column_family = column_family.into();
    let key = key.as_ref().to_vec();
    let value = value.as_ref().to_vec();
    self
      .execute(move |db| {
        let cf = db
          .cf_handle(&column_family)
          .ok_or_else(|| AsyncDbError::MissingColumnFamily(column_family.clone()))?;
        db.put_cf(&cf, key, value)?;
        Ok(())
      })
      .await
  }

  /// Gets a copied value from a named column family.
  ///
  /// # Errors
  ///
  /// Returns an error when the column family is missing or the operation fails.
  pub async fn get_cf(
    &self,
    column_family: impl Into<String>,
    key: impl AsRef<[u8]>,
  ) -> Result<Option<Vec<u8>>, AsyncDbError> {
    let column_family = column_family.into();
    let key = key.as_ref().to_vec();
    self
      .execute(move |db| {
        let cf = db
          .cf_handle(&column_family)
          .ok_or_else(|| AsyncDbError::MissingColumnFamily(column_family.clone()))?;
        Ok(db.get_cf(&cf, key)?)
      })
      .await
  }

  /// Scans up to `limit` entries whose keys start with `prefix`.
  ///
  /// Results follow RocksDB's bytewise key order. Returned keys and values are
  /// copied so no iterator or database borrow crosses an `.await` point.
  ///
  /// # Errors
  ///
  /// Returns an error when the limiter, Tokio task, or iterator fails.
  pub async fn scan_prefix(
    &self,
    prefix: impl AsRef<[u8]>,
    limit: usize,
  ) -> Result<Vec<KeyValue>, AsyncDbError> {
    let prefix = prefix.as_ref().to_vec();
    self
      .execute(move |db| {
        let mut entries = Vec::with_capacity(limit.min(64));
        let mode = IteratorMode::From(&prefix, Direction::Forward);

        for item in db.iterator(mode) {
          let (key, value) = item?;
          if !key.starts_with(&prefix) || entries.len() == limit {
            break;
          }
          entries.push((key.into_vec(), value.into_vec()));
        }
        Ok(entries)
      })
      .await
  }

  /// Returns RocksDB's estimated key count for the default column family.
  ///
  /// # Errors
  ///
  /// Returns an error when the limiter, Tokio task, or property lookup fails.
  pub async fn estimated_key_count(&self) -> Result<Option<u64>, AsyncDbError> {
    self
      .execute(move |db| Ok(db.property_int_value("rocksdb.estimate-num-keys")?))
      .await
  }

  /// Returns the latest RocksDB sequence number.
  ///
  /// # Errors
  ///
  /// Returns an error when the limiter or Tokio task fails.
  pub async fn latest_sequence_number(&self) -> Result<u64, AsyncDbError> {
    self
      .execute(move |db| Ok(db.latest_sequence_number()))
      .await
  }

  /// Flushes the write-ahead log, optionally syncing it to durable storage.
  ///
  /// # Errors
  ///
  /// Returns an error when the limiter, Tokio task, or RocksDB flush fails.
  pub async fn flush_wal(&self, sync: bool) -> Result<(), AsyncDbError> {
    self
      .execute(move |db| {
        db.flush_wal(sync)?;
        Ok(())
      })
      .await
  }

  /// Drops this handle on Tokio's blocking pool.
  ///
  /// If this is the final handle, RocksDB is closed there as well. Cloned
  /// handles keep the database open until they are also dropped.
  ///
  /// # Errors
  ///
  /// Returns an error if Tokio cannot complete the blocking task.
  pub async fn close(self) -> Result<(), AsyncDbError> {
    let inner = self.inner;
    task::spawn_blocking(move || drop(inner)).await?;
    Ok(())
  }

  async fn execute<T, F>(&self, operation: F) -> Result<T, AsyncDbError>
  where
    T: Send + 'static,
    F: FnOnce(&DB) -> Result<T, AsyncDbError> + Send + 'static,
  {
    let permit = Arc::clone(&self.blocking_slots)
      .acquire_owned()
      .await
      .map_err(|_| AsyncDbError::OperationLimiterClosed)?;
    let db = Arc::clone(&self.inner);

    task::spawn_blocking(move || {
      let _permit = permit;
      operation(&db)
    })
    .await?
  }
}

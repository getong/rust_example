use openraft::{
  testing::log::{StoreBuilder, Suite},
  StorageError,
};
use tempfile::TempDir;

use crate::{log_store::RocksLogStore, RocksStateMachine, TypeConfig};

struct RocksBuilder {}

impl StoreBuilder<TypeConfig, RocksLogStore<TypeConfig>, RocksStateMachine, TempDir>
  for RocksBuilder
{
  async fn build(
    &self,
  ) -> Result<(TempDir, RocksLogStore<TypeConfig>, RocksStateMachine), StorageError<TypeConfig>> {
    let td = TempDir::new().expect("couldn't create temp dir");
    let (log_store, sm) = crate::new(td.path()).await;
    Ok((td, log_store, sm))
  }
}

#[tokio::test]
pub async fn test_rocks_store() -> Result<(), StorageError<TypeConfig>> {
  Suite::test_all(RocksBuilder {}).await?;
  Ok(())
}

use openraft::{
  StorageError,
  testing::log::{StoreBuilder, Suite},
  type_config::TypeConfigExt,
};
use tempfile::TempDir;

use crate::{RocksStateMachine, TypeConfig, log_store::RocksLogStore};

struct RocksBuilder {}

impl StoreBuilder<TypeConfig, RocksLogStore<TypeConfig>, RocksStateMachine, TempDir>
  for RocksBuilder
{
  async fn build(
    &self,
  ) -> Result<(TempDir, RocksLogStore<TypeConfig>, RocksStateMachine), StorageError<TypeConfig>> {
    let td = TempDir::new().map_err(|e| StorageError::read(TypeConfig::err_from_error(&e)))?;
    let (log_store, sm) = crate::new(td.path())
      .await
      .map_err(|e| StorageError::read(TypeConfig::err_from_error(&e)))?;
    Ok((td, log_store, sm))
  }
}

#[test]
pub fn test_rocks_store() {
  TypeConfig::run(async {
    Suite::test_all(RocksBuilder {}).await.unwrap();
  });
}

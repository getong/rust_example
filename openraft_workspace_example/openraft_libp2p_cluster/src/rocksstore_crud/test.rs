use openraft::{
  StorageError,
  testing::log::{StoreBuilder, Suite},
  type_config::TypeConfigExt,
};
use tempfile::TempDir;

use super::{RocksStateMachine, TypeConfig, log_store::FjallLogStore};

struct FjallLogRocksStateBuilder {}

impl StoreBuilder<TypeConfig, FjallLogStore<TypeConfig>, RocksStateMachine, TempDir>
  for FjallLogRocksStateBuilder
{
  async fn build(
    &self,
  ) -> Result<(TempDir, FjallLogStore<TypeConfig>, RocksStateMachine), StorageError<TypeConfig>> {
    let td = TempDir::new().map_err(|e| StorageError::read(TypeConfig::err_from_error(&e)))?;
    let (log_store, sm) = super::new(td.path())
      .await
      .map_err(|e| StorageError::read(TypeConfig::err_from_error(&e)))?;
    Ok((td, log_store, sm))
  }
}

#[test]
pub fn test_fjall_log_rocks_state_store() {
  TypeConfig::run(async {
    Suite::test_all(FjallLogRocksStateBuilder {}).await.unwrap();
  });
}

use std::sync::Arc;

use openraft::testing::log::{StoreBuilder, Suite};

use crate::{
  TypeConfig,
  store::{LogStore, StateMachineStore},
  typ::*,
};

struct MemKVStoreBuilder {}

impl StoreBuilder<TypeConfig, LogStore, Arc<StateMachineStore>, ()> for MemKVStoreBuilder {
  async fn build(&self) -> Result<((), LogStore, Arc<StateMachineStore>), StorageError> {
    Ok(((), LogStore::default(), Arc::default()))
  }
}

#[tokio::test]
pub async fn test_mem_store() -> Result<(), StorageError> {
  Suite::test_all(MemKVStoreBuilder {}).await?;
  Ok(())
}

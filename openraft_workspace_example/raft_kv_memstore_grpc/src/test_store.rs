use std::sync::Arc;

use openraft::{
  testing::log::{StoreBuilder, Suite},
  type_config::TypeConfigExt,
};

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

#[test]
pub fn test_mem_store() {
  TypeConfig::run(async {
    Suite::test_all(MemKVStoreBuilder {}).await.unwrap();
  });
}

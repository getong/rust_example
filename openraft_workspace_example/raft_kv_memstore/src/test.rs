use openraft::{
  StorageError,
  testing::log::{StoreBuilder, Suite},
  type_config::TypeConfigExt,
};

use crate::{
  TypeConfig,
  store::{LogStore, StateMachineStore},
};

struct MemKVStoreBuilder {}

impl StoreBuilder<TypeConfig, LogStore<TypeConfig>, StateMachineStore<TypeConfig>, ()>
  for MemKVStoreBuilder
{
  async fn build(
    &self,
  ) -> Result<((), LogStore<TypeConfig>, StateMachineStore<TypeConfig>), StorageError<TypeConfig>>
  {
    Ok(((), LogStore::default(), Default::default()))
  }
}

#[test]
pub fn test_mem_store() {
  TypeConfig::run(async {
    Suite::test_all(MemKVStoreBuilder {}).await.unwrap();
  });
}

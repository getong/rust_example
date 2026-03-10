use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Callback,
  "abi/Callback.json"
);

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  TestStorage,
  "abi/TestStorage.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(callback) = super::deployed_contract!(
    provider,
    Callback,
    "TransientStorage.Callback",
    "TestStorage::Callback"
  ) else {
    return Ok(());
  };
  let Some(storage) = super::deployed_contract!(
    provider,
    TestStorage,
    "TransientStorage.TestStorage",
    "TestStorage"
  ) else {
    return Ok(());
  };
  println!(
    "[TestStorage] callback={}, test_storage={}",
    callback.address(),
    storage.address()
  );

  callback
    .test(*storage.address())
    .send()
    .await?
    .watch()
    .await?;

  let callback_val = callback.val().call().await?;
  let storage_val = storage.val().call().await?;
  println!("[TestStorage] callback.val={callback_val}, testStorage.val={storage_val}");
  Ok(())
}

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
  TestTransientStorage,
  "abi/TestTransientStorage.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(callback) = super::deployed_contract!(
    provider,
    Callback,
    "TransientStorage.Callback",
    "TestTransientStorage::Callback"
  ) else {
    return Ok(());
  };
  let Some(transient) = super::deployed_contract!(
    provider,
    TestTransientStorage,
    "TransientStorage.TestTransientStorage",
    "TestTransientStorage"
  ) else {
    return Ok(());
  };
  println!(
    "[TestTransientStorage] callback={}, test_transient_storage={}",
    callback.address(),
    transient.address()
  );

  callback
    .test(*transient.address())
    .send()
    .await?
    .watch()
    .await?;

  let callback_val = callback.val().call().await?;
  let transient_val = transient.val().call().await?;
  println!("[TestTransientStorage] callback.val={callback_val}, transient.val={transient_val}");
  Ok(())
}

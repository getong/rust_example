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
  let callback = Callback::deploy(provider).await?;
  let transient = TestTransientStorage::deploy(provider).await?;
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

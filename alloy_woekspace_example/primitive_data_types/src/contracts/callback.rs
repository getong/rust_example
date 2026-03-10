use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Callback,
  "abi/Callback.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) =
    super::deployed_contract!(provider, Callback, "TransientStorage.Callback", "Callback")
  else {
    return Ok(());
  };

  let val = contract.val().call().await?;
  println!("[Callback] val = {val}");
  Ok(())
}

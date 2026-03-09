use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Callback,
  "abi/Callback.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Callback::deploy(provider).await?;
  println!("[Callback] deployed: {}", contract.address());

  let val = contract.val().call().await?;
  println!("[Callback] val = {val}");
  Ok(())
}

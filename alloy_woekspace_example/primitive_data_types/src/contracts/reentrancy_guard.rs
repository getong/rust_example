use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  ReentrancyGuard,
  "abi/ReentrancyGuard.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    ReentrancyGuard,
    "TransientStorage.ReentrancyGuard",
    "ReentrancyGuard"
  ) else {
    return Ok(());
  };

  contract.test().send().await?.watch().await?;
  println!("[ReentrancyGuard] test() called successfully");
  Ok(())
}

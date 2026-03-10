use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  MaliciousCallback,
  "abi/MaliciousCallback.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    MaliciousCallback,
    "TransientStorage.MaliciousCallback",
    "MaliciousCallback"
  ) else {
    return Ok(());
  };

  let count = contract.count().call().await?;
  println!("[MaliciousCallback] count={count} (attack() intentionally skipped)");
  Ok(())
}

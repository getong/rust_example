use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  MaliciousCallback,
  "abi/MaliciousCallback.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = MaliciousCallback::deploy(provider).await?;
  println!("[MaliciousCallback] deployed: {}", contract.address());

  let count = contract.count().call().await?;
  println!("[MaliciousCallback] count={count} (attack() intentionally skipped)");
  Ok(())
}

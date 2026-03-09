use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  ReentrancyGuard,
  "abi/ReentrancyGuard.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = ReentrancyGuard::deploy(provider).await?;
  println!("[ReentrancyGuard] deployed: {}", contract.address());

  contract.test().send().await?.watch().await?;
  println!("[ReentrancyGuard] test() called successfully");
  Ok(())
}

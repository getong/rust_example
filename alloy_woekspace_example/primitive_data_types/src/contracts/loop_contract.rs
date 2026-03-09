use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Loop,
  "abi/Loop.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Loop::deploy(provider).await?;
  println!("[Loop] deployed: {}", contract.address());

  contract.r#loop().call().await?;
  println!("[Loop] loop() call succeeded");
  Ok(())
}

use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  ReentrancyGuardTransient,
  "abi/ReentrancyGuardTransient.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = ReentrancyGuardTransient::deploy(provider).await?;
  println!(
    "[ReentrancyGuardTransient] deployed: {}",
    contract.address()
  );

  contract.test().send().await?.watch().await?;
  println!("[ReentrancyGuardTransient] test() called successfully");
  Ok(())
}

use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  ReentrancyGuardTransient,
  "abi/ReentrancyGuardTransient.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    ReentrancyGuardTransient,
    "ReentrancyGuardTransient",
    "ReentrancyGuardTransient"
  ) else {
    return Ok(());
  };

  contract.test().send().await?.watch().await?;
  println!("[ReentrancyGuardTransient] test() called successfully");
  Ok(())
}

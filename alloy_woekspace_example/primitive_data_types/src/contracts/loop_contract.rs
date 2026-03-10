use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Loop,
  "abi/Loop.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(provider, Loop, "Loop.Loop", "Loop") else {
    return Ok(());
  };

  contract.r#loop().call().await?;
  println!("[Loop] loop() call succeeded");
  Ok(())
}

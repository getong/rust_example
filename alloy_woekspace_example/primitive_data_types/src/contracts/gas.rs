use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Gas,
  "abi/Gas.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(provider, Gas, "Gas.Gas", "Gas") else {
    return Ok(());
  };

  let i = contract.i().call().await?;
  println!("[Gas] i = {i} (forever() is intentionally skipped)");
  Ok(())
}

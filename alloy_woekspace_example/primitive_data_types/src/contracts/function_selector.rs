use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  FunctionSelector,
  "abi/FunctionSelector.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    FunctionSelector,
    "FunctionSelector",
    "FunctionSelector"
  ) else {
    return Ok(());
  };

  let selector = contract
    .getSelector("transfer(address,uint256)".to_string())
    .call()
    .await?;
  println!("[FunctionSelector] selector={selector}");
  Ok(())
}

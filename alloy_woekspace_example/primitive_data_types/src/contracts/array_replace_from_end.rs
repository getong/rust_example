use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  ArrayReplaceFromEnd,
  "abi/ArrayReplaceFromEnd.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    ArrayReplaceFromEnd,
    "ArrayReplaceFromEnd",
    "ArrayReplaceFromEnd"
  ) else {
    return Ok(());
  };

  contract.test().send().await?.watch().await?;
  let first = contract.arr(U256::ZERO).call().await?;
  println!("[ArrayReplaceFromEnd] first item after test = {first}");
  Ok(())
}

use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  NestedMapping,
  "abi/NestedMapping.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) =
    super::deployed_contract!(provider, NestedMapping, "NestedMapping", "NestedMapping")
  else {
    return Ok(());
  };

  let who = *contract.address();
  let key = U256::from(1_u64);
  contract.set(who, key, true).send().await?.watch().await?;

  let value = contract.get(who, key).call().await?;
  println!("[NestedMapping] nested[self][1] = {value}");

  contract.remove(who, key).send().await?.watch().await?;
  Ok(())
}

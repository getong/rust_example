use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Mapping,
  "abi/Mapping.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(provider, Mapping, "Mapping", "Mapping") else {
    return Ok(());
  };

  let who = *contract.address();
  contract
    .set(who, U256::from(99_u64))
    .send()
    .await?
    .watch()
    .await?;

  let value = contract.get(who).call().await?;
  println!("[Mapping] myMap[self] = {value}");

  contract.remove(who).send().await?.watch().await?;
  Ok(())
}

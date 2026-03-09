use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  DataLocations,
  "abi/DataLocations.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = DataLocations::deploy(provider).await?;
  println!("[DataLocations] deployed: {}", contract.address());

  contract.f().send().await?.watch().await?;
  contract
    .g(vec![U256::from(1_u64), U256::from(2_u64)])
    .call()
    .await?;

  let first = contract.arr(U256::ZERO).call().await?;
  println!("[DataLocations] arr[0] = {first}");
  Ok(())
}

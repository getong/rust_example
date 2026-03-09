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
    .send()
    .await?
    .watch()
    .await?;
  contract
    .h(vec![U256::from(3_u64), U256::from(4_u64)])
    .send()
    .await?
    .watch()
    .await?;

  println!(
    "[DataLocations] f(), g(...), h(...) executed (arr is never populated in this contract, skip \
     arr(0))"
  );
  Ok(())
}

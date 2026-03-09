use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Counter,
  "abi/Counter.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Counter::deploy(provider).await?;
  println!("[Counter] deployed: {}", contract.address());

  contract
    .setNumber(U256::from(5_u64))
    .send()
    .await?
    .watch()
    .await?;
  contract.increment().send().await?.watch().await?;

  let number = contract.number().call().await?;
  println!("[Counter] number = {number}");
  Ok(())
}

use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Array,
  "abi/Array.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Array::deploy(provider).await?;
  println!("[Array] deployed: {}", contract.address());

  contract
    .push(U256::from(10_u64))
    .send()
    .await?
    .watch()
    .await?;
  contract
    .push(U256::from(20_u64))
    .send()
    .await?
    .watch()
    .await?;

  let len = contract.getLength().call().await?;
  let first = contract.get(U256::ZERO).call().await?;
  println!("[Array] len={len}, first={first}");
  Ok(())
}

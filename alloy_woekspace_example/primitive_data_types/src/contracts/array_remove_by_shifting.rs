use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  ArrayRemoveByShifting,
  "abi/ArrayRemoveByShifting.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = ArrayRemoveByShifting::deploy(provider).await?;
  println!("[ArrayRemoveByShifting] deployed: {}", contract.address());

  contract.test().send().await?.watch().await?;
  let len = contract.arr(U256::ZERO).call().await;
  println!("[ArrayRemoveByShifting] test() executed, arr(0) call result = {len:?}");
  Ok(())
}

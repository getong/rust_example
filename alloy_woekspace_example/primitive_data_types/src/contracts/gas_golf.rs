use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  GasGolf,
  "abi/GasGolf.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = GasGolf::deploy(provider).await?;
  println!("[GasGolf] deployed: {}", contract.address());

  contract
    .sumIfEvenAndLessThan99(vec![
      U256::from(1_u64),
      U256::from(2_u64),
      U256::from(3_u64),
      U256::from(4_u64),
      U256::from(5_u64),
      U256::from(100_u64),
    ])
    .send()
    .await?
    .watch()
    .await?;

  let total = contract.total().call().await?;
  println!("[GasGolf] total={total}");
  Ok(())
}

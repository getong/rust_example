use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  ViewAndPure,
  "abi/ViewAndPure.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = ViewAndPure::deploy(provider).await?;
  println!("[ViewAndPure] deployed: {}", contract.address());

  let x = contract.x().call().await?;
  let add = contract
    .add(U256::from(3_u64), U256::from(4_u64))
    .call()
    .await?;
  let add_to_x = contract.addToX(U256::from(9_u64)).call().await?;
  println!("[ViewAndPure] x={x}, add(3,4)={add}, addToX(9)={add_to_x}");
  Ok(())
}

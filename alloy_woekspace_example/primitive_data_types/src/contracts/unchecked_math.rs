use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  UncheckedMath,
  "abi/UncheckedMath.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    UncheckedMath,
    "UncheckedMath.UncheckedMath",
    "UncheckedMath"
  ) else {
    return Ok(());
  };

  let add_result = contract
    .add(U256::from(5_u64), U256::from(7_u64))
    .call()
    .await?;
  let sub_result = contract
    .sub(U256::from(10_u64), U256::from(4_u64))
    .call()
    .await?;
  let cubes = contract
    .sumOfCubes(U256::from(2_u64), U256::from(3_u64))
    .call()
    .await?;
  println!("[UncheckedMath] add={add_result}, sub={sub_result}, cubes={cubes}");
  Ok(())
}

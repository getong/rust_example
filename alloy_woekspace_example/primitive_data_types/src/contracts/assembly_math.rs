use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  AssemblyMath,
  "abi/AssemblyMath.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = AssemblyMath::deploy(provider).await?;
  println!("[AssemblyMath] deployed: {}", contract.address());

  let add_result = contract
    .yul_add(U256::from(11_u64), U256::from(31_u64))
    .call()
    .await?;
  let mul_result = contract
    .yul_mul(U256::from(7_u64), U256::from(9_u64))
    .call()
    .await?;
  let rounded = contract
    .yul_fixed_point_round(U256::from(90_u64), U256::from(100_u64))
    .call()
    .await?;
  println!(
    "[AssemblyMath] yul_add(11,31)={add_result}, yul_mul(7,9)={mul_result}, \
     yul_fixed_point_round(90,100)={rounded}"
  );
  Ok(())
}

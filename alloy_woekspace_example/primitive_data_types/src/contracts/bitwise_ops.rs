use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  BitwiseOps,
  "abi/BitwiseOps.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(provider, BitwiseOps, "BitwiseOps", "BitwiseOps")
  else {
    return Ok(());
  };

  let and_result = contract
    .and(U256::from(14_u64), U256::from(11_u64))
    .call()
    .await?;
  let xor_result = contract
    .xor(U256::from(12_u64), U256::from(5_u64))
    .call()
    .await?;
  let msb = contract
    .mostSignificantBit(U256::from(12_u64))
    .call()
    .await?;
  println!("[BitwiseOps] and={and_result}, xor={xor_result}, msb(12)={msb}");
  Ok(())
}

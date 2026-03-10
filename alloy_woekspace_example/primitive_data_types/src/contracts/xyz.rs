use alloy::{
  primitives::{Address, U256},
  providers::Provider,
  sol,
};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  XYZ,
  "abi/XYZ.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(provider, XYZ, "XYZ", "XYZ") else {
    return Ok(());
  };

  let call1 = contract.callFunc().call().await?;
  let call2 = contract.callFuncWithKeyValue().call().await?;
  let direct = contract
    .someFuncWithManyInputs(
      U256::from(1_u64),
      U256::from(2_u64),
      U256::from(3_u64),
      Address::ZERO,
      true,
      "demo".to_string(),
    )
    .call()
    .await?;

  println!("[XYZ] callFunc={call1}, callFuncWithKeyValue={call2}, direct={direct}");
  Ok(())
}

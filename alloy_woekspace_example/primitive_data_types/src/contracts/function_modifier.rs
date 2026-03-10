use alloy::{
  primitives::{Address, U256},
  providers::Provider,
  sol,
};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  FunctionModifier,
  "abi/FunctionModifier.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    FunctionModifier,
    "FunctionModifier",
    "FunctionModifier"
  ) else {
    return Ok(());
  };

  contract
    .changeOwner(Address::repeat_byte(0x11))
    .send()
    .await?
    .watch()
    .await?;
  contract
    .decrement(U256::from(1_u64))
    .send()
    .await?
    .watch()
    .await?;

  let owner = contract.owner().call().await?;
  let x = contract.x().call().await?;
  let locked = contract.locked().call().await?;
  println!("[FunctionModifier] owner={owner}, x={x}, locked={locked}");
  Ok(())
}

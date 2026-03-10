use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Enum,
  "abi/EnumBasic.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(provider, Enum, "EnumBasic", "Enum.sol::Enum")
  else {
    return Ok(());
  };

  contract.set(1_u8).send().await?.watch().await?;
  let status = contract.get().call().await?;
  println!("[Enum.sol::Enum] status after set(1) = {status}");

  contract.cancel().send().await?.watch().await?;
  let canceled = contract.status().call().await?;
  println!("[Enum.sol::Enum] status after cancel = {canceled}");
  Ok(())
}

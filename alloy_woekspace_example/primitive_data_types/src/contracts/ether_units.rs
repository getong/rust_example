use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  EtherUnits,
  "abi/EtherUnits.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) =
    super::deployed_contract!(provider, EtherUnits, "EtherUnits.EtherUnits", "EtherUnits")
  else {
    return Ok(());
  };

  let one_ether = contract.oneEther().call().await?;
  let is_one_ether = contract.isOneEther().call().await?;
  println!("[EtherUnits] oneEther={one_ether}, isOneEther={is_one_ether}");
  Ok(())
}

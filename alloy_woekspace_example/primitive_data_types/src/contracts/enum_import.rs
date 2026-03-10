use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Enum,
  "abi/EnumImport.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) =
    super::deployed_contract!(provider, Enum, "EnumImport.Enum", "EnumImport.sol::Enum")
  else {
    return Ok(());
  };

  let status = contract.status().call().await?;
  println!("[EnumImport.sol::Enum] default status = {status}");
  Ok(())
}

use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Enum,
  "abi/EnumImport.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Enum::deploy(provider).await?;
  println!("[EnumImport.sol::Enum] deployed: {}", contract.address());

  let status = contract.status().call().await?;
  println!("[EnumImport.sol::Enum] default status = {status}");
  Ok(())
}

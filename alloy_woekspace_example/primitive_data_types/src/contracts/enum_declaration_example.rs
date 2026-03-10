use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  EnumDeclarationExample,
  "abi/EnumDeclarationExample.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    EnumDeclarationExample,
    "EnumDeclarationExample",
    "EnumDeclarationExample"
  ) else {
    return Ok(());
  };

  contract.set(2_u8).send().await?.watch().await?;
  let status = contract.status().call().await?;
  println!("[EnumDeclarationExample] status = {status}");
  Ok(())
}

use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  EnumDeclarationExample,
  "abi/EnumDeclarationExample.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = EnumDeclarationExample::deploy(provider).await?;
  println!("[EnumDeclarationExample] deployed: {}", contract.address());

  contract.set(2_u8).send().await?.watch().await?;
  let status = contract.status().call().await?;
  println!("[EnumDeclarationExample] status = {status}");
  Ok(())
}

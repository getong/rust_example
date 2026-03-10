use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  AssemblyLoop,
  "abi/AssemblyLoop.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    AssemblyLoop,
    "AssemblyLoop.AssemblyLoop",
    "AssemblyLoop"
  ) else {
    return Ok(());
  };

  let for_count = contract.yul_for_loop().call().await?;
  let while_count = contract.yul_while_loop().call().await?;
  println!("[AssemblyLoop] yul_for_loop()={for_count}, yul_while_loop()={while_count}");
  Ok(())
}

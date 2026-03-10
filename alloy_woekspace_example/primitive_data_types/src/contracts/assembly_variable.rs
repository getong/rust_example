use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  AssemblyVariable,
  "abi/AssemblyVariable.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    AssemblyVariable,
    "AssemblyVariable",
    "AssemblyVariable"
  ) else {
    return Ok(());
  };

  let result = contract.yul_let().call().await?;
  println!("[AssemblyVariable] yul_let()={result}");
  Ok(())
}

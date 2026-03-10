use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  AssemblyError,
  "abi/AssemblyError.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    AssemblyError,
    "AssemblyError.AssemblyError",
    "AssemblyError"
  ) else {
    return Ok(());
  };

  contract.yul_revert(U256::from(7_u64)).call().await?;
  println!("[AssemblyError] yul_revert(7) completed without revert");
  Ok(())
}

use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  AssemblyIf,
  "abi/AssemblyIf.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) =
    super::deployed_contract!(provider, AssemblyIf, "AssemblyIf.AssemblyIf", "AssemblyIf")
  else {
    return Ok(());
  };

  let if_result = contract.yul_if(U256::from(5_u64)).call().await?;
  let switch_result = contract.yul_switch(U256::from(2_u64)).call().await?;
  println!("[AssemblyIf] yul_if(5)={if_result}, yul_switch(2)={switch_result}");
  Ok(())
}

use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  IfElse,
  "abi/IfElse.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = IfElse::deploy(provider).await?;
  println!("[IfElse] deployed: {}", contract.address());

  let foo = contract.foo(U256::from(15_u64)).call().await?;
  let ternary = contract.ternary(U256::from(9_u64)).call().await?;
  println!("[IfElse] foo(15)={foo}, ternary(9)={ternary}");
  Ok(())
}

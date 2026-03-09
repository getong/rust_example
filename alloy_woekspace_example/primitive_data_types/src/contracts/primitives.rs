use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Primitives,
  "abi/Primitives.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Primitives::deploy(provider).await?;
  println!("[Primitives] deployed: {}", contract.address());

  let boo = contract.boo().call().await?;
  let u256_val = contract.u256().call().await?;
  let i_val = contract.i().call().await?;
  let dynamic_len = contract.dynamicBytesLength().call().await?;
  println!("[Primitives] boo={boo}, u256={u256_val}, i={i_val}, dynamicBytesLen={dynamic_len}");
  Ok(())
}

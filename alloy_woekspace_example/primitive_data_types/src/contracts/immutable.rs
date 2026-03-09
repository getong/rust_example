use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Immutable,
  "abi/Immutable.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Immutable::deploy(provider, U256::from(123_u64)).await?;
  println!("[Immutable] deployed: {}", contract.address());
  Ok(())
}

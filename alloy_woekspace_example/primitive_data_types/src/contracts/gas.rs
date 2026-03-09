use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Gas,
  "abi/Gas.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Gas::deploy(provider).await?;
  println!("[Gas] deployed: {}", contract.address());

  let i = contract.i().call().await?;
  println!("[Gas] i = {i} (forever() is intentionally skipped)");
  Ok(())
}

use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Constants,
  "abi/Constants.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Constants::deploy(provider).await?;
  println!("[Constants] deployed: {}", contract.address());
  Ok(())
}

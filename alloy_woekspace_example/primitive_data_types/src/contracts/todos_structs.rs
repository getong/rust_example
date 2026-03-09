use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Todos,
  "abi/TodosStructs.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Todos::deploy(provider).await?;
  println!("[Structs.sol::Todos] deployed: {}", contract.address());
  println!("[Structs.sol::Todos] this example only deploys (no mutating methods exposed)");
  Ok(())
}

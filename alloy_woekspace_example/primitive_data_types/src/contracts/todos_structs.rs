use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Todos,
  "abi/TodosStructs.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(_contract) =
    super::deployed_contract!(provider, Todos, "TodosStructs", "Structs.sol::Todos")
  else {
    return Ok(());
  };
  println!("[Structs.sol::Todos] this example only deploys (no mutating methods exposed)");
  Ok(())
}

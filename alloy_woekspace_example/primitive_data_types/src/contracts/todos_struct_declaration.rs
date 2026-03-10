use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Todos,
  "abi/TodosStructDeclaration.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    Todos,
    "StructDeclaration.Todos",
    "StructDeclaration.sol::Todos"
  ) else {
    return Ok(());
  };

  contract
    .create("buy milk".to_string())
    .send()
    .await?
    .watch()
    .await?;
  contract
    .updateText(U256::ZERO, "buy coffee".to_string())
    .send()
    .await?
    .watch()
    .await?;
  contract
    .toggleCompleted(U256::ZERO)
    .send()
    .await?
    .watch()
    .await?;

  let todo = contract.get(U256::ZERO).call().await?;
  println!(
    "[StructDeclaration.sol::Todos] todo[0] = {{ text: {}, completed: {} }}",
    todo.text, todo.completed
  );
  Ok(())
}

use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  StructImportExample,
  "abi/StructImportExample.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    StructImportExample,
    "StructImport.StructImportExample",
    "StructImportExample"
  ) else {
    return Ok(());
  };

  contract
    .set("read book".to_string(), true)
    .send()
    .await?
    .watch()
    .await?;

  let todo = contract.todo().call().await?;
  println!(
    "[StructImportExample] todo = {{ text: {}, completed: {} }}",
    todo.text, todo.completed
  );
  Ok(())
}

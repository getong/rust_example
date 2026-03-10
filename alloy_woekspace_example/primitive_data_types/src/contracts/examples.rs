use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Examples,
  "abi/Examples.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(provider, Examples, "Examples", "Examples") else {
    return Ok(());
  };

  contract.exampleNoUvdt().call().await?;
  contract.exampleUvdt().call().await?;
  println!("[Examples] exampleNoUvdt() and exampleUvdt() called");
  Ok(())
}

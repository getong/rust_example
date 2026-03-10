use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Function,
  "abi/Function.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) =
    super::deployed_contract!(provider, Function, "Function.Function", "Function")
  else {
    return Ok(());
  };

  contract.returnMany().call().await?;
  contract.named().call().await?;
  println!("[Function] returnMany() and named() called");
  Ok(())
}

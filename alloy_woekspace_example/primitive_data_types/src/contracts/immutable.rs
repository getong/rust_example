use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Immutable,
  "abi/Immutable.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(_contract) = super::deployed_contract!(provider, Immutable, "Immutable", "Immutable")
  else {
    return Ok(());
  };
  Ok(())
}

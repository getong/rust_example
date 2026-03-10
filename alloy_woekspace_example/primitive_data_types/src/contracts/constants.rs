use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Constants,
  "abi/Constants.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(_contract) = super::deployed_contract!(provider, Constants, "Constants", "Constants")
  else {
    return Ok(());
  };
  Ok(())
}

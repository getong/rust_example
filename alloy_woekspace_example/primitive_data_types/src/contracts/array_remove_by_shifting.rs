use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  ArrayRemoveByShifting,
  "abi/ArrayRemoveByShifting.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    ArrayRemoveByShifting,
    "ArrayRemoveByShifting",
    "ArrayRemoveByShifting"
  ) else {
    return Ok(());
  };

  contract.test().send().await?.watch().await?;
  println!(
    "[ArrayRemoveByShifting] test() executed (arr is empty after test, skip arr(0) to avoid \
     bounds revert)"
  );
  Ok(())
}

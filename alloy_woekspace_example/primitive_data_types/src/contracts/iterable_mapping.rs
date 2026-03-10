use alloy::{primitives::U256, providers::Provider, sol};
use eyre::{Result, ensure};

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  TestIterableMap,
  "abi/TestIterableMap.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let caller = provider
    .get_accounts()
    .await?
    .first()
    .copied()
    .ok_or_else(|| eyre::eyre!("no unlocked account available from provider"))?;

  let Some(contract) = super::deployed_contract!(
    provider,
    TestIterableMap,
    "IterableMapping.TestIterableMap",
    "IterableMapping::TestIterableMap"
  ) else {
    return Ok(());
  };

  contract
    .setInMapping(U256::from(55_u64))
    .send()
    .await?
    .watch()
    .await?;
  let size_after_set = contract.sizeOfMapping().call().await?;
  let value = contract.getFromMap().call().await?;
  let first_key = contract.getKeyAtIndex(U256::ZERO).call().await?;
  ensure!(
    first_key == caller,
    "expected first iterable-map key to equal caller"
  );

  contract.removeFromMapping().send().await?.watch().await?;
  let size_after_remove = contract.sizeOfMapping().call().await?;
  println!(
    "[IterableMapping] value={value}, size_after_set={size_after_set}, \
     size_after_remove={size_after_remove}"
  );
  Ok(())
}

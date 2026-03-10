use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Payable,
  "abi/Payable.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Payable::deploy(provider).await?;
  println!("[Payable] deployed: {}", contract.address());

  contract
    .deposit()
    .value(U256::from(2_u64))
    .send()
    .await?
    .watch()
    .await?;

  let owner = contract.owner().call().await?;
  let balance_before = provider.get_balance(*contract.address()).await?;
  contract.withdraw().send().await?.watch().await?;
  let balance_after = provider.get_balance(*contract.address()).await?;
  println!(
    "[Payable] owner={owner}, balance_before={balance_before}, balance_after={balance_after}"
  );
  Ok(())
}

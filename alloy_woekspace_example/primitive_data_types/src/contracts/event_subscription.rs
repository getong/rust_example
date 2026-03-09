use alloy::{primitives::Address, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  EventSubscription,
  "abi/EventSubscription.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = EventSubscription::deploy(provider).await?;
  println!("[EventSubscription] deployed: {}", contract.address());

  contract.subscribe().send().await?.watch().await?;
  let caller = provider
    .get_accounts()
    .await?
    .first()
    .copied()
    .unwrap_or(Address::ZERO);

  let subscribed = contract.subscribers(caller).call().await?;
  println!("[EventSubscription] subscribers[{caller}] = {subscribed}");
  Ok(())
}

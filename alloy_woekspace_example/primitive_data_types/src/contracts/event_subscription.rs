use alloy::{primitives::U256, providers::Provider, sol};
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
  contract
    .transfer(*contract.address(), U256::from(7_u64))
    .send()
    .await?
    .watch()
    .await?;
  contract.unsubscribe().send().await?.watch().await?;

  let subscribed = contract.subscribers(*contract.address()).call().await?;
  println!("[EventSubscription] subscribers[self] = {subscribed}");
  Ok(())
}

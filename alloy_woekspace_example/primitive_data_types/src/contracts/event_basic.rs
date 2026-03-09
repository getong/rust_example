use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Event,
  "abi/Event.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Event::deploy(provider).await?;
  println!("[Events.sol::Event] deployed: {}", contract.address());

  contract.test().send().await?.watch().await?;
  println!("[Events.sol::Event] emitted logs via test()");
  Ok(())
}

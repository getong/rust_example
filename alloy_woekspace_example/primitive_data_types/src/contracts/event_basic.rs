use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Event,
  "abi/Event.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) =
    super::deployed_contract!(provider, Event, "Events.Event", "Events.sol::Event")
  else {
    return Ok(());
  };

  contract.test().send().await?.watch().await?;
  println!("[Events.sol::Event] emitted logs via test()");
  Ok(())
}

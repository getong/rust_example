use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Fallback,
  "abi/Fallback.json"
);

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  SendToFallback,
  "abi/SendToFallback.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(fallback) =
    super::deployed_contract!(provider, Fallback, "Fallback.Fallback", "Fallback")
  else {
    return Ok(());
  };
  let Some(sender) = super::deployed_contract!(
    provider,
    SendToFallback,
    "Fallback.SendToFallback",
    "Fallback::SendToFallback"
  ) else {
    return Ok(());
  };
  println!(
    "[Fallback] receiver: {}, sender: {}",
    fallback.address(),
    sender.address()
  );

  sender
    .callFallback(*fallback.address())
    .value(U256::from(1_u64))
    .send()
    .await?
    .watch()
    .await?;
  sender
    .transferToFallback(*fallback.address())
    .value(U256::from(1_u64))
    .send()
    .await?
    .watch()
    .await?;

  let balance = fallback.getBalance().call().await?;
  println!("[Fallback] balance={balance}");
  Ok(())
}

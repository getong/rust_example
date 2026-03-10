use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  ReceiveEther,
  "abi/ReceiveEther.json"
);

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  SendEther,
  "abi/SendEther.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(receiver) = super::deployed_contract!(
    provider,
    ReceiveEther,
    "SendingEther.ReceiveEther",
    "SendingEther::ReceiveEther"
  ) else {
    return Ok(());
  };
  let Some(sender) = super::deployed_contract!(
    provider,
    SendEther,
    "SendingEther.SendEther",
    "SendingEther::SendEther"
  ) else {
    return Ok(());
  };
  println!(
    "[SendingEther] receiver: {}, sender: {}",
    receiver.address(),
    sender.address()
  );

  sender
    .sendViaCall(*receiver.address())
    .value(U256::from(1_u64))
    .send()
    .await?
    .watch()
    .await?;
  sender
    .sendViaSend(*receiver.address())
    .value(U256::from(1_u64))
    .send()
    .await?
    .watch()
    .await?;

  let balance = receiver.getBalance().call().await?;
  println!("[SendingEther] receiver_balance={balance}");
  Ok(())
}

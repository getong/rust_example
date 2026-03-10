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
  let receiver = ReceiveEther::deploy(provider).await?;
  let sender = SendEther::deploy(provider).await?;
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

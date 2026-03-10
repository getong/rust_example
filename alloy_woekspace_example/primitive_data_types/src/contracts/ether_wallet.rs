use alloy::{primitives::U256, providers::Provider, rpc::types::TransactionRequest, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  EtherWallet,
  "abi/EtherWallet.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = EtherWallet::deploy(provider).await?;
  println!("[EtherWallet] deployed: {}", contract.address());

  provider
    .send_transaction(
      TransactionRequest::default()
        .to(*contract.address())
        .value(U256::from(1_u64)),
    )
    .await?
    .watch()
    .await?;

  let balance_before = contract.getBalance().call().await?;
  contract
    .withdraw(balance_before)
    .send()
    .await?
    .watch()
    .await?;
  let balance_after = contract.getBalance().call().await?;
  println!("[EtherWallet] balance_before={balance_before}, balance_after={balance_after}");
  Ok(())
}

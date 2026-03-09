use alloy::{
  primitives::{B256, U256},
  providers::Provider,
  sol,
};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  EventDrivenArchitecture,
  "abi/EventDrivenArchitecture.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = EventDrivenArchitecture::deploy(provider).await?;
  println!("[EventDrivenArchitecture] deployed: {}", contract.address());

  contract
    .initiateTransfer(*contract.address(), U256::from(123_u64))
    .send()
    .await?
    .watch()
    .await?;

  let transfer_id = B256::with_last_byte(1);
  contract
    .confirmTransfer(transfer_id)
    .send()
    .await?
    .watch()
    .await?;

  let confirmed = contract.transferConfirmations(transfer_id).call().await?;
  println!("[EventDrivenArchitecture] transferConfirmations[{transfer_id}] = {confirmed}");
  Ok(())
}

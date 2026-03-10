use alloy::{
  network::ReceiptResponse,
  primitives::{Address, U256},
  providers::Provider,
  sol,
};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  EventSubscription,
  "abi/EventSubscription.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    EventSubscription,
    "EventSubscription",
    "EventSubscription"
  ) else {
    return Ok(());
  };

  // Call transfer(address,uint256) from src/EventsAdvanced.sol first.
  let transfer_pending = contract
    .transfer(Address::repeat_byte(0x22), U256::from(7_u64))
    .send()
    .await?;
  let transfer_receipt = transfer_pending.get_receipt().await?;
  transfer_receipt.ensure_success()?;
  println!(
    "[EventSubscription] transfer tx={}, status={}, gas_used={}",
    transfer_receipt.transaction_hash(),
    transfer_receipt.status(),
    transfer_receipt.gas_used()
  );

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

//! Example showing how to build a transaction using the `TransactionBuilder`

use alloy::{
  network::TransactionBuilder,
  node_bindings::Anvil,
  primitives::U256,
  providers::{Provider, ProviderBuilder},
  rpc::types::TransactionRequest,
};
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
  // Spin up a local Anvil node.
  // Ensure `anvil` is available in $PATH.
  let anvil = Anvil::new().try_spawn()?;

  // Create a provider.
  let rpc_url = anvil.endpoint().parse()?;
  let provider = ProviderBuilder::new().connect_http(rpc_url);

  // Create two users, Alice and Bob.
  let alice = anvil.addresses()[0];
  let bob = anvil.addresses()[1];

  // Build a transaction to send 100 wei from Alice to Bob.
  // The `from` field is automatically filled to the first signer's address (Alice).
  let tx = TransactionRequest::default()
    .with_from(alice)
    .with_to(bob)
    .with_nonce(0)
    .with_chain_id(anvil.chain_id())
    .with_value(U256::from(100))
    .with_gas_limit(21_000)
    .with_max_priority_fee_per_gas(1_000_000_000)
    .with_max_fee_per_gas(20_000_000_000);

  // https://alloy.rs/highlights/the-transaction-lifecycle.html?highlight=timeout#signing-and-broadcasting-the-transaction
  // Send the transaction and wait for the broadcast.
  // The Provider::send_transaction method returns a PendingTransactionBuilder for
  // configuring the pending transaction watcher.
  // On it we can for example, set the required_confirmations or set a timeout:
  // Configure the pending transaction.
  let pending_tx = provider
    .send_transaction(tx)
    .await?
    .with_required_confirmations(2)
    .with_timeout(Some(std::time::Duration::from_secs(60)));

  println!("Pending transaction... {}", pending_tx.tx_hash());

  // Wait for the transaction to be included and get the receipt.
  let receipt = pending_tx.get_receipt().await?;

  println!(
    "Transaction included in block {}",
    receipt.block_number.expect("Failed to get block number")
  );

  assert_eq!(receipt.from, alice);
  assert_eq!(receipt.to, Some(bob));

  Ok(())
}

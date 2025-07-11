use std::time::Duration;

use solana_client::{
  pubsub_client::PubsubClient,
  rpc_client::RpcClient,
  rpc_config::{RpcAccountInfoConfig, RpcSendTransactionConfig},
};
use solana_sdk::{
  commitment_config::CommitmentConfig,
  native_token::LAMPORTS_PER_SOL,
  signature::{Keypair, Signer},
  transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  println!("🚀 Starting Solana Account Subscribe Example");

  // Use localhost devnet (local test validator)
  let ws_url = String::from("ws://127.0.0.1:8900/");
  let rpc_url = String::from("http://127.0.0.1:8899");

  // Create RPC client for transactions
  let rpc_client = RpcClient::new(rpc_url);

  // Create a test keypair that we'll monitor
  let test_keypair = Keypair::new();
  let pubkey = test_keypair.pubkey();

  println!("📧 Created test account: {}", pubkey);

  // Check if local validator is running
  match rpc_client.get_balance(&pubkey) {
    Ok(_) => println!("✅ Connected to local validator"),
    Err(e) => {
      println!("❌ Failed to connect to local validator: {}", e);
      println!("💡 Make sure to start local validator first:");
      println!("   solana-test-validator");
      return Err(e.into());
    }
  }

  // Subscribe to account changes
  println!("🔗 Subscribing to account changes...");
  let (mut _client, receiver) = PubsubClient::account_subscribe(
    &ws_url,
    &pubkey,
    Some(RpcAccountInfoConfig {
      encoding: None,
      data_slice: None,
      commitment: Some(CommitmentConfig::confirmed()),
      min_context_slot: None,
    }),
  )?;

  println!("👂 Listening for account changes...");

  // Start a background task to receive subscription messages
  let pubkey_clone = pubkey.clone();
  let handle = tokio::spawn(async move {
    loop {
      match receiver.recv() {
        Ok(message) => {
          println!(
            "📬 Received account update for {}: {:?}",
            pubkey_clone, message
          );
        }
        Err(e) => {
          println!("❌ Error receiving message: {}", e);
          break;
        }
      }
    }
  });

  // Give subscription time to establish
  tokio::time::sleep(Duration::from_secs(2)).await;

  // Create a funding account to send SOL from
  let funding_keypair = Keypair::new();
  let funding_pubkey = funding_keypair.pubkey();

  println!("💰 Requesting airdrop for funding account...");
  let airdrop_signature = rpc_client.request_airdrop(&funding_pubkey, LAMPORTS_PER_SOL)?;

  // Wait for airdrop to be confirmed
  loop {
    match rpc_client.get_signature_status(&airdrop_signature)? {
      Some(Ok(())) => {
        println!("✅ Airdrop confirmed!");
        break;
      }
      Some(Err(e)) => {
        println!("❌ Airdrop failed: {}", e);
        return Err(e.into());
      }
      None => {
        println!("⏳ Waiting for airdrop confirmation...");
        tokio::time::sleep(Duration::from_secs(2)).await;
      }
    }
  }

  // Send some SOL to the monitored account to trigger subscription
  println!("💸 Sending SOL to monitored account to trigger subscription...");
  let transfer_amount = LAMPORTS_PER_SOL / 10; // 0.1 SOL
  let transfer_instruction =
    system_instruction::transfer(&funding_pubkey, &pubkey, transfer_amount);

  let recent_blockhash = rpc_client.get_latest_blockhash()?;
  let transaction = Transaction::new_signed_with_payer(
    &[transfer_instruction],
    Some(&funding_pubkey),
    &[&funding_keypair],
    recent_blockhash,
  );

  let signature = rpc_client.send_and_confirm_transaction_with_spinner_and_config(
    &transaction,
    CommitmentConfig::confirmed(),
    RpcSendTransactionConfig::default(),
  )?;

  println!("✅ Transfer completed! Signature: {}", signature);

  // Wait a bit to receive the subscription update
  tokio::time::sleep(Duration::from_secs(5)).await;

  // Send another transaction to trigger another update
  println!("💸 Sending another transaction to trigger more updates...");
  let transfer_instruction2 =
    system_instruction::transfer(&funding_pubkey, &pubkey, transfer_amount);

  let recent_blockhash2 = rpc_client.get_latest_blockhash()?;
  let transaction2 = Transaction::new_signed_with_payer(
    &[transfer_instruction2],
    Some(&funding_pubkey),
    &[&funding_keypair],
    recent_blockhash2,
  );

  let signature2 = rpc_client.send_and_confirm_transaction_with_spinner_and_config(
    &transaction2,
    CommitmentConfig::confirmed(),
    RpcSendTransactionConfig::default(),
  )?;

  println!("✅ Second transfer completed! Signature: {}", signature2);

  // Wait for final updates
  tokio::time::sleep(Duration::from_secs(5)).await;

  println!("🏁 Example completed! Check the account updates above.");

  // Cancel the subscription task
  handle.abort();

  Ok(())
}

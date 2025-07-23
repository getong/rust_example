use std::sync::Arc;

use pumpfun::{
  PumpFun,
  // accounts::BondingCurveAccount,
  common::types::{Cluster, PriorityFee},
  utils::CreateTokenMetadata,
};
use solana_sdk::{
  commitment_config::CommitmentConfig, native_token::sol_str_to_lamports, signature::Keypair,
  signer::Signer,
};

#[tokio::main]
async fn main() {
  // Create a new PumpFun client
  let payer = Arc::new(Keypair::new());
  let client = PumpFun::new(
    payer.clone(),
    Cluster::localnet(CommitmentConfig::finalized(), PriorityFee::default()),
  );

  // Mint keypair
  let mint = Keypair::new();

  // Token metadata
  let metadata = CreateTokenMetadata {
    name: "Lorem ipsum".to_string(),
    symbol: "LIP".to_string(),
    description: "Lorem ipsum dolor, sit amet consectetur adipisicing elit. Quam, nisi."
      .to_string(),
    file: "image.png".to_string(),
    twitter: None,
    telegram: None,
    website: Some("https://example.com".to_string()),
  };

  // Optional priority fee to expedite transaction processing (e.g., 100 LAMPORTS per compute unit,
  // equivalent to a 0.01 SOL priority fee)
  let fee = Some(PriorityFee {
    unit_limit: Some(100_000),
    unit_price: Some(100_000_000),
  });

  // Create token with metadata
  let signature = client
    .create(mint.insecure_clone(), metadata.clone(), fee)
    .await
    .unwrap();
  println!("Create signature: {}", signature);

  // Create and buy tokens with metadata
  let signature = client
    .create_and_buy(
      mint.insecure_clone(),
      metadata.clone(),
      sol_str_to_lamports("1f64").unwrap(),
      None,
      fee,
    )
    .await
    .unwrap();
  println!("Created and buy signature: {}", signature);

  // Print the curve
  let curve = client
    .get_bonding_curve_account(&mint.pubkey())
    .await
    .unwrap();
  println!("Bonding curve: {:#?}", curve);

  // Buy tokens (ATA will be created automatically if needed)
  let signature = client
    .buy(
      mint.pubkey(),
      sol_str_to_lamports("1f64").unwrap(),
      None,
      fee,
    )
    .await
    .unwrap();
  println!("Buy signature: {}", signature);

  // Sell tokens (sell all tokens)
  let signature = client.sell(mint.pubkey(), None, None, fee).await.unwrap();
  println!("Sell signature: {}", signature);

  // Subscribe to real-time events with the stream feature
  use pumpfun::common::stream::PumpFunEvent;

  // Subscribe to Pump.fun events
  let _subscription = client
    .subscribe(None, |signature, event, error, _response| match event {
      Some(PumpFunEvent::Create(create_event)) => {
        println!(
          "New token created: {} ({})",
          create_event.name, create_event.symbol
        );
        println!("Mint address: {}", create_event.mint);
        println!("Created by: {}", create_event.creator);
      }
      Some(PumpFunEvent::Trade(trade_event)) => {
        let action = if trade_event.is_buy { "bought" } else { "sold" };
        println!(
          "User {} {} {} tokens for {} SOL",
          trade_event.user,
          action,
          trade_event.token_amount,
          trade_event.sol_amount as f64 / 1_000_000_000.0
        );
      }
      Some(event) => println!("Other event received: {:#?}", event),
      None => {
        if let Some(err) = error {
          eprintln!("Error parsing event in tx {}: {}", signature, err);
        }
      }
    })
    .await
    .unwrap();

  // Keep subscription active as long as needed
  // The subscription will automatically unsubscribe when dropped
}

// copy from https://docs.rs/pumpfun/latest/pumpfun/
// git clone https://github.com/nhuxhr/pumpfun-rs
// cd pumpfun-rs/scripts
// ./pumpfun-test-validator.sh

// in another window
// cargo run

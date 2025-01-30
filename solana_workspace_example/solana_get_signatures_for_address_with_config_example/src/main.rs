use std::str::FromStr;

use solana_client::{
  rpc_client::{GetConfirmedSignaturesForAddress2Config, RpcClient},
  rpc_response::RpcConfirmedTransactionStatusWithSignature,
};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Connect to a Solana cluster
  let client = RpcClient::new("https://api.mainnet-beta.solana.com");

  // Define the address to query
  let address = Pubkey::from_str("YourPublicKeyHere")?;

  // Set the configuration for the request
  let config = GetConfirmedSignaturesForAddress2Config {
    before: None,
    until: None,
    limit: Some(3),
    commitment: Some(CommitmentConfig::confirmed()),
  };

  // Call the get_signatures_for_address_with_config RPC method
  let signatures: Vec<RpcConfirmedTransactionStatusWithSignature> = client
    .get_signatures_for_address_with_config(&address, config)
    .map_err(|_| Box::<dyn std::error::Error>::from("not found"))?;

  // Display the results
  for signature in signatures {
    println!("Signature: {}", signature.signature);
    println!("Slot: {}", signature.slot);
    println!("block time: {:?}", signature.block_time);
  }

  Ok(())
}

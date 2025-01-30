use std::str::FromStr;

use solana_client::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use solana_transaction_status::UiTransactionEncoding;

#[tokio::main]
async fn main() {
  // Replace with your actual Solana RPC URL
  let rpc_url = "https://api.mainnet-beta.solana.com";
  let client = RpcClient::new(rpc_url);

  // Replace with your actual public key string
  let address = "YourPublicKeyHere";

  // Parsing the public key string to Pubkey
  let pubkey = Pubkey::from_str(address).expect("Invalid public key");

  // Fetching the transaction signatures for the address
  let signatures = client
    .get_signatures_for_address(&pubkey)
    .expect("Failed to get signatures");

  println!("Signatures for address {}:", address);
  for sig in signatures {
    println!("{}", sig.signature);

    // Fetching the transaction details for each signature
    match client.get_transaction(
      &Signature::from_str(&sig.signature).unwrap(),
      UiTransactionEncoding::Json,
    ) {
      Ok(transaction) => {
        println!("Transaction details: {:?}", transaction);
      }
      Err(e) => {
        eprintln!("Error fetching transaction: {}", e);
      }
    }
  }
}

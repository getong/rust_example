use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
fn main() {
  // Create an RPC client to connect to the Solana cluster
  let rpc_url = "https://api.mainnet-beta.solana.com";
  let client = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

  // Fetch the current slot
  match client.get_slot() {
    Ok(slot) => {
      println!("Current slot: {}", slot);
    }
    Err(err) => {
      eprintln!("Error fetching current slot: {}", err);
    }
  }
}

use solana_client::rpc_client::RpcClient;

fn main() {
  // Create an RPC client to interact with the Solana cluster
  let rpc_url = "https://api.mainnet-beta.solana.com";
  let client = RpcClient::new(rpc_url.to_string());

  // Fetch the genesis block hash
  match client.get_genesis_hash() {
    Ok(genesis_hash) => {
      println!("Genesis block hash: {}", genesis_hash);

      // Fetch the genesis block using the hash
      match client.get_block(0) {
        Ok(genesis_block) => {
          println!("Genesis block: {:?}", genesis_block);
        }
        Err(err) => {
          eprintln!("Error fetching genesis block: {}", err);
        }
      }
    }
    Err(err) => {
      eprintln!("Error fetching genesis block hash: {}", err);
    }
  }
}

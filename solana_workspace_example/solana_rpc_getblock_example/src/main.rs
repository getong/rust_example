use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;

fn main() {
    // Create an RPC client to connect to the Solana cluster
    let rpc_url = "https://api.mainnet-beta.solana.com";
    let client = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    // Specify the slot number of the block you want to fetch
    let slot = 12345678;

    // Fetch the block
    match client.get_block(slot) {
        Ok(block) => {
            println!("Block at slot {}: {:?}", slot, block);
        }
        Err(err) => {
            eprintln!("Error fetching block: {}", err);
        }
    }
}

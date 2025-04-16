use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ethereum JSON-RPC endpoint
    let url = "https://base-rpc.publicnode.com";

    // JSON-RPC request to get the latest block number
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_blockNumber",
        "params": [],
        "id": 1
    });

    let client = Client::new();
    let response = client
        .post(url)
        .json(&payload)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    // Parse the hexadecimal block number
    if let Some(result) = response["result"].as_str() {
        let block_number = u64::from_str_radix(result.trim_start_matches("0x"), 16)?;
        println!("Latest Ethereum block height: {}", block_number);
    } else {
        println!("Error: {:?}", response);
    }

    Ok(())
}

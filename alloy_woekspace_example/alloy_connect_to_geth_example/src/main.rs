use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Define the Geth JSON-RPC endpoint
  let url = "http://127.0.0.1:8545";

  // Create a new HTTP client
  let client = Client::new();

  // JSON-RPC request payload for `eth_blockNumber`
  let payload = json!({
      "jsonrpc": "2.0",
      "method": "eth_blockNumber",
      "params": [],
      "id": 1
  });

  // Send the request to Geth
  let response = client
    .post(url)
    .json(&payload)
    .send()
    .await?
    .json::<serde_json::Value>()
    .await?;

  // Print the response (latest block number)
  println!("Response: {:?}", response);

  Ok(())
}

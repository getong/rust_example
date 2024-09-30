use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let url = "http://127.0.0.1:8545";
  let client = Client::new();

  let payload = json!({
      "jsonrpc": "2.0",
      "method": "eth_sendTransaction",
      "params": [{
          "from": "0xYourAccountAddress",
          "to": "0xRecipientAddress",
          "value": "0x1",  // Amount of Wei to send
          "gas": "0x5208",  // Gas limit
          "gasPrice": "0x3b9aca00"  // Gas price (in Wei)
      }],
      "id": 1
  });

  let response = client
    .post(url)
    .json(&payload)
    .send()
    .await?
    .json::<serde_json::Value>()
    .await?;

  println!("Response: {:?}", response);

  Ok(())
}

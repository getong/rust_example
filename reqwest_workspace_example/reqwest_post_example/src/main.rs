use std::error::Error;

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, USER_AGENT};
use serde_json::{json, Value};

// TODO change your domain and check args
const CHECK_URL: &str = "check_domain";
const CHECK_ARGS: &str = "check_args";

fn construct_headers() -> HeaderMap {
  let mut headers = HeaderMap::new();
  headers.insert(USER_AGENT, HeaderValue::from_static("reqwest"));
  headers.insert(CONTENT_TYPE, HeaderValue::from_static("image/png"));
  headers
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  _ = complicated_example().await?;

  let client = reqwest::Client::new();

  let json_body = json!({
      "jsonrpc": "2.0",
      "id": 1,
      "method": "getAccountInfo",
      "params": [
          "F65xgN7bUhaJffKhtFotQsZEgd8DLviWt59qBMA4LfC5",
          {
              "encoding": "base58"
          }
      ]

  });

  let res = client
    .post("https://api.devnet.solana.com")
    .headers(construct_headers())
    .json(&json_body)
    .send()
    .await
    .expect("failed to get response")
    .text()
    .await
    .expect("failed to get payload");

  println!("{:?}", res);

  Ok(())
}

async fn complicated_example() -> Result<(), Box<dyn std::error::Error>> {
  let client = reqwest::Client::new();

  let query = json!({
      // "query": format!("{{\n  indexers(filter: {{controller: {{equalToInsensitive: \"{}\"}}}}) {{\n    nodes {{\n      id\n    }}\n  }}\n}}", CHECK_ARGS)
      "query": format!("{{\n  stateChannels(filter: {{id: {{equalTo: \"0x{}\"}}}}) {{\n    nodes {{\n      id\n    }}\n  }}\n}}", CHECK_ARGS)
  });

  let response = client.post(CHECK_URL).json(&query).send().await?;

  let body = response.text().await?;
  println!("graphql query result: {}", body);

  let v: Value = serde_json::from_str(&body)?;
  if let Some(id) = v
    .get("data")
    .and_then(|data| data.get("indexers"))
    .and_then(|indexers| indexers.get("nodes"))
    .and_then(|nodes| nodes.get(0))
    .and_then(|node| node.get("id"))
    .and_then(|id| id.as_str())
  {
    println!("ID: {}", id);
    Ok(())
  } else {
    println!("ID not found");
    Err("ID not found".into())
  }
}

// copy from https://github.com/serpentacademy/http-post-request-in-rust-to-solana-api

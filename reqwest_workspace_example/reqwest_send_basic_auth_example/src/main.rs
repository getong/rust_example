use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// work with ../../../axum_workspace_example/axum_basic_auth_example/
#[derive(Serialize, Deserialize)]
struct MyStruct {
  a: String,
  b: Option<String>,
  c: Option<String>,
  d: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let client = Client::new();

  // Set up the path parameter
  let channel_id = "my_channel";

  // Set up the query parameters
  let params = MyStruct {
    a: "your_api_key".to_string(),
    b: Some("my_channel".to_string()),
    c: Some("my_ep_name".to_string()),
    d: Some("multiple".to_string()),
  };

  // Serialize query parameters to HashMap
  let mut query_params = HashMap::new();
  query_params.insert("a", params.a);
  if let Some(b) = params.b {
    query_params.insert("b", b);
  }
  if let Some(c) = params.c {
    query_params.insert("c", c);
  }
  if let Some(d) = params.d {
    query_params.insert("d", d);
  }

  // Authorization string
  let auth = "your_auth_token";

  // Build the request URL
  let url = format!("http://localhost:3000/state/{}", channel_id);

  // Send the POST request, use body method
  let response = client
    .post(&url)
    .header("auth", auth)
    .body(auth)
    .query(&query_params)
    .send()
    .await?;

  println!("response: {:?}\n", response);

  // Handle the response
  let response_json: Value = response.json().await?;
  println!("Response: {:?}", response_json);

  // Send the POST request, use json method
  let response = client
    .post(&url)
    .header("auth", auth)
    .json(auth)
    .query(&query_params)
    .send()
    .await?;

  println!("response: {:?}\n", response);

  // Handle the response
  let response_json: Value = response.json().await?;
  println!("Response: {:?}", response_json);

  Ok(())
}

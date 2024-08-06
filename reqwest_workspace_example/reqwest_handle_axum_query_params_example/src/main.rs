use reqwest::Client;
use std::collections::HashMap;

// work with ../../../axum_workspace_example/axum_handle_parameter_example
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let client = Client::new();

  // Set up the params
  let params = Params {
    apikey: "MMIB0PIZYC76AN665IIW2MY8".to_string(),
    channel: None,
    ep_name: None,
    block: Some("multiple".to_string()),
  };

  // Convert params to a HashMap
  let mut params_map = HashMap::new();
  params_map.insert("apikey", params.apikey);
  if let Some(channel) = params.channel {
    params_map.insert("channel", channel);
  }
  if let Some(ep_name) = params.ep_name {
    params_map.insert("ep_name", ep_name);
  }
  if let Some(block) = params.block {
    params_map.insert("block", block);
  }

  // Build the URL with query parameters
  let project = "your_project";
  let url = format!("http://localhost:3000/sign/{}", project);
  let request_url = reqwest::Url::parse_with_params(&url, &params_map)?;

  // Send the POST request
  let response = client.post(request_url).send().await?;

  // Handle the response
  let response_text = response.text().await?;
  println!("Response: {}", response_text);

  Ok(())
}

// Define the Params struct used in reqwest client
#[derive(Debug)]
struct Params {
  apikey: String,
  channel: Option<String>,
  ep_name: Option<String>,
  block: Option<String>,
}

use reqwest::Client;
use std::collections::HashMap;

// work with ../../../axum_workspace_example/axum_handle_parameter_example
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let client = Client::new();

  // Convert params to a HashMap
  let mut params_map = HashMap::new();
  params_map.insert("apikey", "fakecode".to_string());
  params_map.insert("block", "multiple".to_string());

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

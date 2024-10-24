// src/main.rs
fn main() {
  let api_key = std::env::var("API_KEY").expect("API_KEY not found");
  let api_url = std::env::var("API_URL").expect("API_URL not found");
  let api_token = std::env::var("API_TOKEN").expect("API_TOKEN not found");

  println!("API_KEY: {}", api_key);
  println!("API_URL: {}", api_url);
  println!("API_TOKEN: {}", api_token);
}

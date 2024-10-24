// build.rs
fn main() {
  // Load the .env file
  dotenv::from_filename(".env").ok();

  // Retrieve environment variables
  if let Ok(api_key) = std::env::var("API_KEY") {
    println!("cargo:rustc-env=API_KEY={}", api_key);
  }

  if let Ok(api_url) = std::env::var("API_URL") {
    println!("cargo:rustc-env=API_URL={}", api_url);
  }
}

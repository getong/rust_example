// build.rs
fn main() {
  // Load the .env file
  dotenv::dotenv().ok();

  // Set the SENTRY_DSN environment variable to be available at compile time
  if let Ok(sentry_dsn) = std::env::var("SENTRY_DSN") {
    println!("cargo:rustc-env=SENTRY_DSN={}", sentry_dsn);
  }
}

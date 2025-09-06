// Simple CLI loader example
mod simple_cli_loader;

use deno_error::JsErrorBox;

#[tokio::main]
async fn main() -> Result<(), JsErrorBox> {
  env_logger::init();

  println!("Starting Simple CLI Module Loader Example");
  println!("=========================================\n");

  simple_cli_loader::run_simple_cli_example().await?;

  println!("\n=========================================");
  println!("Example completed successfully!");

  Ok(())
}

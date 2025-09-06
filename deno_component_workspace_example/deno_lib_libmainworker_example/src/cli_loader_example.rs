// Example using the CLI-inspired module loader
mod cli_based_module_loader;

use deno_error::JsErrorBox;

#[tokio::main]
async fn main() -> Result<(), JsErrorBox> {
  // Run the CLI-inspired module loader example
  cli_based_module_loader::run_cli_inspired_example().await?;

  Ok(())
}

// Example using the integrated module loader with components from Deno CLI
mod file_fetcher;
mod graph_container;
mod integrated_module_loader;
mod loader_utils;

use deno_error::JsErrorBox;

#[tokio::main]
async fn main() -> Result<(), JsErrorBox> {
  // Initialize logging
  env_logger::init();

  println!("Starting Integrated Module Loader Example");
  println!("=========================================\n");

  // Run the integrated module loader example
  integrated_module_loader::run_integrated_example().await?;

  println!("\n=========================================");
  println!("Example completed successfully!");

  Ok(())
}

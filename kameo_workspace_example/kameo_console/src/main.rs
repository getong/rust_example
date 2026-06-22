// cargo install kameo_console
// kameo-console
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let _console = kameo::console::serve("127.0.0.1:9999").await?;
  println!("serving console on 127.0.0.1:9999 — connect with `cargo run -p kameo_console`");

  // Spawn the demo actor system and keep it alive until ctrl-c.
  let _system = kameo::console::demo::spawn().await;

  tokio::signal::ctrl_c().await?;
  Ok(())
}

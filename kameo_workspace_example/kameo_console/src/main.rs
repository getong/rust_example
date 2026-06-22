// cargo install kameo_console
// kameo-console
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let console = kameo::console::serve("127.0.0.1:9999").await?;
  println!(
    "serving console on {} - connect with `kameo-console` cli",
    console.local_addr()
  );

  // Spawn the demo actor system and keep it alive until ctrl-c.
  let _system = kameo::console::demo::spawn().await;

  tokio::signal::ctrl_c().await?;
  Ok(())
}

use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    signal::ctrl_c().await?;
    println!("ctrl-c received!");
    Ok(())
}
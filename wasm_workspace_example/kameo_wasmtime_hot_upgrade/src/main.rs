mod demo;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
  demo::run().await
}

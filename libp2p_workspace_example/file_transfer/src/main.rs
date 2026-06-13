#[tokio::main]
async fn main() -> anyhow::Result<()> {
  file_transfer::run().await
}

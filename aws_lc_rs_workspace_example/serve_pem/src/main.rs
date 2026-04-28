#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  serve_pem::run().await
}

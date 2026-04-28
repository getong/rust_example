#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
  serve_pem::run().await
}

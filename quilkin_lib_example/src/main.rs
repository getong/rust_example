use clap::Parser as _;

#[tokio::main]
async fn main() -> quilkin::Result<()> {
  stable_eyre::install().unwrap();

  match quilkin::Cli::parse().drive(None).await {
    Ok(()) => std::process::exit(0),
    Err(error) => {
      tracing::error!(%error, error_debug=?error, "fatal error");
      std::process::exit(-1)
    }
  }
}

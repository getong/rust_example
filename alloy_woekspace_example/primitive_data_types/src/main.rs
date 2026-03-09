use std::env;

use alloy::{
  network::EthereumWallet,
  providers::{Provider, ProviderBuilder},
  signers::local::PrivateKeySigner,
};
use eyre::Result;

mod contracts;

const DEFAULT_ANVIL_RPC: &str = "http://127.0.0.1:8545";
const DEFAULT_ANVIL_PRIVATE_KEY: &str =
  "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

#[tokio::main]
async fn main() -> Result<()> {
  let rpc_url = env::var("RPC_URL").unwrap_or_else(|_| DEFAULT_ANVIL_RPC.to_string());
  let private_key = env::var("PRIVATE_KEY").unwrap_or_else(|_| {
    println!(
      "PRIVATE_KEY not set, fallback to default Anvil key. This only works for local Anvil-like \
       nodes."
    );
    DEFAULT_ANVIL_PRIVATE_KEY.to_string()
  });

  if env::var("PRIVATE_KEY").is_err() && !is_local_endpoint(&rpc_url) {
    eyre::bail!(
      "PRIVATE_KEY is required for non-local RPC endpoints. Set both RPC_URL and PRIVATE_KEY."
    );
  }

  let signer: PrivateKeySigner = private_key.parse()?;
  let wallet = EthereumWallet::from(signer);

  let provider = ProviderBuilder::new()
    .wallet(wallet)
    .connect_http(rpc_url.parse()?);

  println!("connected rpc url: {rpc_url}");

  let chain_id = provider.get_chain_id().await?;
  println!("connected chain id: {chain_id}");

  contracts::run_all(&provider).await?;

  println!("all primitive_data_types contract examples finished successfully");
  Ok(())
}

fn is_local_endpoint(rpc_url: &str) -> bool {
  rpc_url.contains("127.0.0.1") || rpc_url.contains("localhost")
}

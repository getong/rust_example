use std::env;

use alloy::{
  network::EthereumWallet,
  primitives::U256,
  providers::{Provider, ProviderBuilder},
  signers::local::PrivateKeySigner,
  sol,
};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Counter,
  "abi/Counter.json"
);

const DEFAULT_ANVIL_RPC: &str = "http://127.0.0.1:8545";
const DEFAULT_ANVIL_PRIVATE_KEY: &str =
  "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

fn is_local_endpoint(rpc_url: &str) -> bool {
  rpc_url.contains("127.0.0.1") || rpc_url.contains("localhost")
}

#[tokio::main]
async fn main() -> Result<()> {
  // Launch a local Anvil chain and use the first default account as signer.
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

  let chain_id = provider.get_chain_id().await?;
  println!("Connected chain id: {chain_id}");

  // Same constructor as Solidity: `constructor(uint256 initialNumber)`.
  let contract = Counter::deploy(&provider, U256::from(11_u64)).await?;
  println!("Counter deployed at: {}", contract.address());

  let number_after_deploy = contract.number().call().await?;
  println!("number after deploy: {number_after_deploy}");

  contract
    .setNumber(U256::from(43_u64))
    .send()
    .await?
    .watch()
    .await?;
  let number_after_set = contract.number().call().await?;
  println!("number after setNumber(43): {number_after_set}");

  contract.increment().send().await?.watch().await?;
  let number_after_increment = contract.number().call().await?;
  println!("number after increment(): {number_after_increment}");

  contract
    .resetToNetworkDefault()
    .send()
    .await?
    .watch()
    .await?;
  let number_after_reset = contract.number().call().await?;
  println!("number after resetToNetworkDefault(): {number_after_reset}");

  // In config_example, chain id 31337 (Anvil) maps to initialNumber = 7.
  assert_eq!(number_after_reset, U256::from(7_u64));
  println!("OK: reset value matches network config (7 on Anvil)");

  Ok(())
}

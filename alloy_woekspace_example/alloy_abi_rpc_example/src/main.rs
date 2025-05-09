use alloy::{
  primitives::{address, utils::format_units},
  providers::{Provider, ProviderBuilder},
  sol,
};

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  RIF,
  "abi/rif.json"
);

#[tokio::main]
async fn main() -> eyre::Result<()> {
  // Set up the HTTP transport which is consumed by the RPC client.
  let rpc_url = "https://rpc.testnet.rootstock.io/{YOUR_APIKEY}".parse()?;

  // Create a provider with the HTTP transport using the `reqwest` crate.
  // Fix the deprecated method by using connect_http instead of on_http
  let provider = ProviderBuilder::new().connect_http(rpc_url);

  // Address without 0x prefix
  let alice = address!("8F1C0185bB6276638774B9E94985d69D3CDB444a");

  let rbtc = provider.get_balance(alice).await?;

  let formatted_balance: String = format_units(rbtc, "ether")?;

  println!("Balance of alice: {formatted_balance} rbtc");

  // Using rif testnet contract address
  let contract = RIF::new(
    "0x19f64674D8a5b4e652319F5e239EFd3bc969a1FE".parse()?,
    provider,
  );

  // Fix the pattern match to handle the Uint type correctly
  let balance = contract.balanceOf(alice).call().await?;
  println!("Rif balance: {:?}", balance);

  Ok(())
}

// copy from https://dev.rootstock.io/resources/tutorials/rootstock-rust/

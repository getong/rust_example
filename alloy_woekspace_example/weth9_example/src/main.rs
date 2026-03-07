use std::env;

use alloy::{
  primitives::Address,
  providers::{Provider, ProviderBuilder},
  sol,
};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  WETH9,
  "abi/WETH9.json"
);

#[tokio::main]
async fn main() -> Result<()> {
  // Use your own Ethereum mainnet HTTPS endpoint.
  // Example:
  // MAINNET_RPC_URL="https://mainnet.infura.io/v3/<key>" cargo run -p weth9_example
  let rpc_url = env::var("MAINNET_RPC_URL")
    .or_else(|_| env::var("mainnetHTTPS"))
    .unwrap_or_else(|_| "https://eth.drpc.org".to_owned());

  let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);

  let chain_id = provider.get_chain_id().await?;
  println!("Connected chain id: {chain_id}");
  if chain_id != 1 {
    panic!("RPC is not Ethereum mainnet (expected chain id 1)");
  }

  // Mainnet WETH9 canonical address.
  let weth_address: Address = env::var("WETH9_ADDRESS")
    .unwrap_or_else(|_| "0xC02aaA39b223FE8D0A0E5C4F27eAD9083C756Cc2".to_owned())
    .parse()?;

  let weth = WETH9::new(weth_address, &provider);
  println!("WETH9 address: {weth_address}");

  // Basic token metadata and supply (read-only).
  let name = weth.name().call().await?;
  let symbol = weth.symbol().call().await?;
  let decimals = weth.decimals().call().await?;
  let total_supply = weth.totalSupply().call().await?;

  println!("name: {name}");
  println!("symbol: {symbol}");
  println!("decimals: {decimals}");
  println!("totalSupply(raw): {total_supply}");

  // Optional: query any holder address.
  // Example:
  // WETH_QUERY_ADDRESS="0x..." cargo run -p weth9_example
  if let Ok(holder) = env::var("WETH_QUERY_ADDRESS") {
    let holder: Address = holder.parse()?;
    let holder_balance = weth.balanceOf(holder).call().await?;
    println!("balanceOf({holder}) = {holder_balance}");
  } else {
    println!("Tip: set WETH_QUERY_ADDRESS to query a holder balance.");
  }

  // Optional: query allowance(owner, spender).
  // Example:
  // WETH_OWNER="0x..." WETH_SPENDER="0x..." cargo run -p weth9_example
  match (env::var("WETH_OWNER"), env::var("WETH_SPENDER")) {
    (Ok(owner), Ok(spender)) => {
      let owner: Address = owner.parse()?;
      let spender: Address = spender.parse()?;
      let allowance = weth.allowance(owner, spender).call().await?;
      println!("allowance({owner}, {spender}) = {allowance}");
    }
    _ => {
      println!("Tip: set WETH_OWNER and WETH_SPENDER to query allowance.");
    }
  }

  Ok(())
}

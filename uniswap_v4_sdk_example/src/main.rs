use alloy::{
  network::EthereumWallet,
  primitives::{address, U256},
  providers::ProviderBuilder,
  signers::local::PrivateKeySigner,
};
use eyre::Result;
use uniswap_sdk_core::entities::{BaseCurrency, BaseCurrencyCore};
use uniswap_v4_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
  dotenv::dotenv().ok();

  println!("Uniswap V4 SDK Example\n");

  let rpc_url = std::env::var("RPC_URL")
    .unwrap_or_else(|_| "https://eth-mainnet.g.alchemy.com/v2/your-api-key".to_string());

  let private_key = std::env::var("PRIVATE_KEY").unwrap_or_else(|_| {
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string()
  });

  let signer: PrivateKeySigner = private_key.parse()?;
  let wallet = EthereumWallet::from(signer);

  let _provider = ProviderBuilder::new()
    .wallet(wallet)
    .connect_http(rpc_url.parse()?);

  let chain_id = 1u64;

  example_pool_info(chain_id).await?;

  example_swap_quote(chain_id).await?;

  example_position_info(chain_id).await?;

  Ok(())
}

async fn example_pool_info(chain_id: u64) -> Result<()> {
  println!("=== Pool Information Example ===");

  let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
  let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");

  let token0 = sdk_core::entities::token::Token::new(
    chain_id,
    weth,
    18,
    Some("WETH".to_string()),
    Some("Wrapped Ether".to_string()),
    0,
    0,
  );

  let token1 = sdk_core::entities::token::Token::new(
    chain_id,
    usdc,
    6,
    Some("USDC".to_string()),
    Some("USD Coin".to_string()),
    0,
    0,
  );

  println!(
    "Token Pair: {} / {}",
    token0.symbol().map_or("???", |s| s.as_str()),
    token1.symbol().map_or("???", |s| s.as_str())
  );
  println!("Token0 Address: {}", token0.address());
  println!("Token1 Address: {}", token1.address());
  println!();

  Ok(())
}

async fn example_swap_quote(chain_id: u64) -> Result<()> {
  println!("=== Swap Quote Example ===");

  let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
  let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");

  let token_in = sdk_core::entities::token::Token::new(
    chain_id,
    weth,
    18,
    Some("WETH".to_string()),
    Some("Wrapped Ether".to_string()),
    0,
    0,
  );

  let token_out = sdk_core::entities::token::Token::new(
    chain_id,
    usdc,
    6,
    Some("USDC".to_string()),
    Some("USD Coin".to_string()),
    0,
    0,
  );

  let amount_in = U256::from(1_000_000_000_000_000_000u64); // 1 WETH

  println!(
    "Swap: 1 {} for {}",
    token_in.symbol().map_or("???", |s| s.as_str()),
    token_out.symbol().map_or("???", |s| s.as_str())
  );
  println!("Amount In: {} wei", amount_in);

  let fee_tier = 3000u32;
  println!("Fee Tier: {}bps (0.3%)", fee_tier / 100);
  println!();

  Ok(())
}

async fn example_position_info(chain_id: u64) -> Result<()> {
  println!("=== Position Management Example ===");

  let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
  let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");

  let token0 = sdk_core::entities::token::Token::new(
    chain_id,
    weth,
    18,
    Some("WETH".to_string()),
    Some("Wrapped Ether".to_string()),
    0,
    0,
  );

  let token1 = sdk_core::entities::token::Token::new(
    chain_id,
    usdc,
    6,
    Some("USDC".to_string()),
    Some("USD Coin".to_string()),
    0,
    0,
  );

  println!(
    "Creating position for {} / {} pool",
    token0.symbol().map_or("???", |s| s.as_str()),
    token1.symbol().map_or("???", |s| s.as_str())
  );

  let tick_lower = -887220;
  let tick_upper = 887220;

  println!("Tick Range: {} to {}", tick_lower, tick_upper);
  println!("This represents a wide price range for the position");

  let liquidity = U256::from(1_000_000_000_000_000u64);
  println!("Liquidity: {}", liquidity);
  println!();

  Ok(())
}

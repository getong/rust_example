use std::{sync::Arc, time::Duration};

use ethers::{
  core::types::{Block, BlockNumber, H256},
  providers::{Http, HttpRateLimitRetryPolicy, Middleware, Provider, RetryClientBuilder},
};
use eyre::Report;
use url::Url;

const RPC_URL: &str = "https://eth.llamarpc.com";

#[tokio::main]
async fn main() -> eyre::Result<()> {
  // Create the base HTTP provider
  let provider = Http::new(Url::parse(RPC_URL)?);

  // Create the retry client
  let retry_client = RetryClientBuilder::default()
    .rate_limit_retries(0)
    .timeout_retries(0)
    .initial_backoff(Duration::from_millis(1))
    .build(provider, Box::<HttpRateLimitRetryPolicy>::default());

  // Wrap the retry client in an `Arc` for sharing
  let provider = Arc::new(Provider::new(retry_client));

  // Send a raw JSON-RPC request for the latest block
  let block_num = "latest".to_string();
  let txn_details = false;
  let params = (block_num, txn_details);

  let block: Block<H256> = provider.request("eth_getBlockByNumber", params).await?;

  println!("\n{block:?}\n");

  // Use the `get_block` method from the `Middleware` trait
  let latest_block = provider
    .get_block(BlockNumber::Latest)
    .await
    .map_err(|e| Report::msg(format!("Failed to get latest block: {e}")))?
    .ok_or_else(|| Report::msg("Block not found"))?
    .number
    .ok_or_else(|| Report::msg("Block number is missing"))?
    .as_u64();

  println!("latest_block: {}", latest_block);

  Ok(())
}

// https://www.gakonst.com/ethers-rs/providers/retry.html
// modified with chatgpt

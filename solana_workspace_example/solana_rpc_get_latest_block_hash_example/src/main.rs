use std::{thread::sleep, time::Duration};

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_transaction_status_client_types::EncodedConfirmedBlock;
#[tokio::main]
async fn main() {
  let rpc_url = "https://api.mainnet-beta.solana.com"; // Use your preferred RPC provider
  let client = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

  match get_latest_block_data(&client).await {
    Ok((block_hash, block_height, block_time)) => {
      println!("Latest Block Hash: {}", block_hash);
      println!("Latest Block Height: {}", block_height);
      if let Some(time) = block_time {
        println!("Latest Block Time: {}", time);
      } else {
        println!("Block time unavailable.");
      }
    }
    Err(err) => eprintln!("Error fetching latest block data: {}", err),
  }

  match client.get_latest_blockhash_with_commitment(CommitmentConfig::confirmed()) {
    Ok((hash, height)) => {
      println!("hash is {:?}, height is {:?}", hash, height);
      let slot = client.get_slot().unwrap();
      println!("slot is {}", slot);
      let block_time = client.get_block_time(slot).unwrap();
      println!("block_time is {:?}", block_time);
    }
    Err(err) => println!("err is {:?}", err),
  }
}

async fn get_latest_block_data(
  client: &RpcClient,
) -> Result<(String, u64, Option<i64>), Box<dyn std::error::Error>> {
  // Get latest block hash
  let latest_block_hash = client.get_latest_blockhash()?;

  // Get latest block height
  let latest_block_height = client.get_block_height()?;

  // Attempt to get a valid block with a timestamp
  let block_info = get_latest_confirmed_block(client, 5).await?;

  Ok((
    latest_block_hash.to_string(),
    latest_block_height,
    block_info.block_time,
  ))
}

async fn get_latest_confirmed_block(
  client: &RpcClient,
  max_retries: usize,
) -> Result<EncodedConfirmedBlock, Box<dyn std::error::Error>> {
  let mut retries = 0;
  let mut slot = client.get_slot()?; // Start with the latest slot

  while retries < max_retries {
    match client.get_block(slot) {
      Ok(block) => return Ok(block), // Found a valid block
      Err(err) => {
        eprintln!(
          "Block not available for slot {}: {}. Retrying...",
          slot, err
        );
        sleep(Duration::from_millis(500)); // Small delay before retrying
        slot = slot.saturating_sub(1); // Try an earlier slot
        retries += 1;
      }
    }
  }

  Err("Failed to retrieve a confirmed block after multiple attempts".into())
}

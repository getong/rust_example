use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use ethers::{
  middleware::Middleware, // Add this import
  providers::{Http, Provider, ProviderError},
};
use eyre::Result;
use futures_timer::Delay;

// List of RPC endpoints to try
const ENDPOINTS: [&str; 2] = [
  "https://base-sepolia.gateway.tenderly.co",
  "https://sepolia.base.org",
];

// Timeout in seconds
const TIMEOUT_SECONDS: u64 = 3;

async fn get_block_size_with_timeout(
  provider: Arc<Provider<Http>>,
) -> Result<usize, ProviderError> {
  let start = Instant::now();

  // Create a future for the block request
  let block_future = provider.get_block(0);

  // Race between the request and a timeout
  let timeout = Delay::new(Duration::from_secs(TIMEOUT_SECONDS));

  tokio::select! {
      result = block_future => {
          match result {
              Ok(Some(block)) => {
                  // Fix: access 'size' as a field, not a method
                  let size = block.size.unwrap_or_default().as_u64() as usize;
                  println!("Request completed in {:?}. Block size: {} bytes", start.elapsed(), size);
                  Ok(size)
              },
              Ok(None) => {
                  println!("Block not found");
                  Err(ProviderError::CustomError("Block not found".into()))
              },
              Err(e) => {
                  println!("Error fetching block: {}", e);
                  Err(e)
              }
          }
      },
      _ = timeout => {
          println!("Request timed out after {}s", TIMEOUT_SECONDS);
          Err(ProviderError::CustomError(format!("Request timed out after {}s", TIMEOUT_SECONDS)))
      }
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  println!("Attempting to get block size from available endpoints...");

  let mut success = false;

  for endpoint in ENDPOINTS.iter() {
    println!("Trying endpoint: {}", endpoint);

    match Provider::<Http>::try_from(endpoint.to_string()) {
      Ok(provider) => {
        let provider = Arc::new(provider);

        match get_block_size_with_timeout(provider.clone()).await {
          Ok(size) => {
            println!(
              "Successfully got block size from {}: {} bytes",
              endpoint, size
            );
            success = true;
            break;
          }
          Err(e) => {
            println!("Failed to get block size from {}: {}", endpoint, e);
            // Continue to the next endpoint
          }
        }
      }
      Err(e) => {
        println!("Failed to create provider for {}: {}", endpoint, e);
        // Continue to the next endpoint
      }
    }
  }

  if !success {
    println!("Failed to get block size from any endpoint");
  }

  Ok(())
}

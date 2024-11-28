use std::time::Duration;

use ethers::{
  providers::{Http, Middleware, Provider, ProviderError},
  types::U256,
};
use tokio::time::sleep;

async fn retry_with_fallback(endpoints: &[&str], max_retries: u32) -> Result<U256, ProviderError> {
  let mut retries = 0;

  for endpoint in endpoints {
    let provider = Provider::<Http>::try_from(*endpoint);

    match provider {
      Ok(provider) => {
        loop {
          match provider.get_block_number().await {
            Ok(block_number) => return Ok(U256::from(block_number.as_u64())), /* Convert `U64`
                                                                                * to `U256` */
            Err(err) if retries < max_retries => {
              // Retry after a delay
              retries += 1;
              eprintln!("Error: {}. Retrying {}/{}...", err, retries, max_retries);
              sleep(Duration::from_secs(1)).await;
            }
            Err(err) => {
              // Move to the next endpoint if max retries are exceeded
              eprintln!(
                "Failed after {} retries on endpoint {}: {}",
                retries, endpoint, err
              );
              break;
            }
          }
        }
      }
      Err(err) => {
        eprintln!(
          "Failed to create provider for endpoint {}: {}",
          endpoint, err
        );
        continue; // Try the next endpoint if this one fails to initialize
      }
    }
  }

  // If all endpoints fail, return a custom `ProviderError`
  Err(ProviderError::CustomError(
    "All endpoints failed".to_string(),
  ))
}

#[tokio::main]
async fn main() {
  let endpoints = &[
    "https://base-sepolia.gateway.tenderly.co",
    "https://base-sepolia.blockpi.network/v1/rpc/public",
    "https://base-sepolia-rpc.publicnode.com",
  ];

  match retry_with_fallback(endpoints, 3).await {
    Ok(block_number) => println!("Latest block number: {:?}", block_number),
    Err(e) => eprintln!("All endpoints failed: {:?}", e),
  }
}

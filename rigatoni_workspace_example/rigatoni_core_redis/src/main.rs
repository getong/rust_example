use std::time::Duration;

use rigatoni_core::pipeline::{DistributedLockConfig, Pipeline, PipelineConfig};
use rigatoni_destinations::s3::{S3Config, S3Destination};
use rigatoni_stores::redis::{RedisConfig, RedisStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Configure Redis store (required for distributed locking)
  let redis_config = RedisConfig::builder()
    .url("redis://localhost:6379")
    .pool_size(10)
    .build()?;

  let store = RedisStore::new(redis_config).await?;

  // Configure S3 destination
  let s3_config = S3Config::builder()
    .bucket("my-data-lake")
    .region("us-east-1")
    .prefix("mongodb-cdc")
    .build()?;

  let destination = S3Destination::new(s3_config).await?;

  // Configure pipeline with distributed locking
  let config = PipelineConfig::builder()
    .mongodb_uri("mongodb://localhost:27017/?replicaSet=rs0")
    .database("mydb")
    .watch_collections(vec!["users".to_string(), "orders".to_string()])
    .distributed_lock(DistributedLockConfig {
      enabled: true,
      ttl: Duration::from_secs(30), // Lock expires if holder crashes
      refresh_interval: Duration::from_secs(10), // Heartbeat interval
      retry_interval: Duration::from_secs(5), // Retry claiming locks
    })
    .build()?;

  // Create and run pipeline
  let mut pipeline = Pipeline::new(config, store, destination).await?;
  pipeline.start().await?;

  Ok(())
}

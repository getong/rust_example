use std::path::PathBuf;

use anyhow::Result;
use rust_rocksdb_basic_example::run_demo;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
  let path = std::env::args_os()
    .nth(1)
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("target/rocksdb-demo"));

  run_demo(path).await
}

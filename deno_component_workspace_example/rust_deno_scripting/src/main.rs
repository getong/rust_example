mod execution;
mod extension;
mod loader;
mod npm_cache;
mod npm_downloader;
mod npm_registry;
mod npm_specifier;

use std::{env, sync::Arc};

use anyhow::{Result, bail};
use extension::HostState;
use tokio::sync::RwLock;

fn main() -> Result<()> {
  // Initialize the default crypto provider for rustls
  if rustls::crypto::CryptoProvider::get_default().is_none() {
    let _ = rustls::crypto::ring::default_provider().install_default();
  }

  let args: Vec<_> = env::args().collect();
  let (file_path, fn_name) = match &args[..] {
    [_, file_path, fn_name] => (file_path, fn_name.as_str()),
    [_, file_path] => (file_path, "main"),
    [binary, ..] => bail!("usage: {binary} <file_path> [<fn_name>]"),
    _ => unreachable!(),
  };

  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()?;
  runtime.block_on(run(file_path, fn_name))
}

async fn run(file_path: &str, fn_name: &str) -> Result<()> {
  let host_state = Arc::new(RwLock::new(HostState { n: 42 }));
  println!("host state: {}", host_state.read().await.n);
  execution::run_js(file_path, fn_name, host_state.clone()).await?;
  println!("host state: {}", host_state.read().await.n);

  Ok(())
}

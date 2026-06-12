#[cfg(target_arch = "wasm32")]
mod guest;
#[cfg(not(target_arch = "wasm32"))]
mod host;

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
  host::run().await
}

#[cfg(target_arch = "wasm32")]
#[wstd::main]
async fn main() -> anyhow::Result<()> {
  guest::run().await
}

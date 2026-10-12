//! QUIC 应用服务端：注册 ALPN 与 Echo handler，relay 只负责底层转发。
use anyhow::{Context, Result};
use iroh::protocol::Router;

use crate::{
  cli::Network,
  endpoint,
  protocol::{ALPN, Echo},
};

pub async fn spawn(network: &Network) -> Result<Router> {
  let endpoint = endpoint::bind(network).await?;
  Ok(Router::builder(endpoint).accept(ALPN, Echo).spawn())
}

pub async fn run(network: &Network) -> Result<()> {
  let router = spawn(network).await?;
  println!("server EndpointId: {}", router.endpoint().id());
  println!(
    "mode: {}",
    if network.allow_direct {
      "relay + UDP direct"
    } else {
      "relay-only"
    }
  );
  println!(
    "cargo run -p iroh_relay_example -- client --relay-url {} --server {} --message '你好 QUIC'",
    network.relay_url,
    router.endpoint().id()
  );
  println!("按 Ctrl+C 停止 server");
  let result = tokio::signal::ctrl_c().await.context("监听 Ctrl+C 失败");
  let shutdown = router.shutdown().await.context("关闭 QUIC server 失败");
  result.and(shutdown)
}

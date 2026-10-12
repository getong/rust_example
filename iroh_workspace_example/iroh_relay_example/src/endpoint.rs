//! 两端共享网络设置：只配置指定 relay，默认禁用 IP 直连。
use std::time::Duration;

use anyhow::{Context, Result};
use iroh::{Endpoint, RelayMode, endpoint::presets};
use iroh_relay::{RelayConfig, RelayMap};

use crate::{cli::Network, protocol::ALPN};

pub const OPERATION_TIMEOUT: Duration = Duration::from_secs(20);

pub async fn bind(network: &Network) -> Result<Endpoint> {
  // 显式禁用 relay 的 QAD 探测，与本地 HTTP relay 配置保持一致。
  let relays = RelayMap::from(RelayConfig::new(network.relay_url.clone(), None));
  let mut builder = Endpoint::builder(presets::Minimal)
    .alpns(vec![ALPN.to_vec()])
    .relay_mode(RelayMode::Custom(relays));
  if !network.allow_direct {
    builder = builder.clear_ip_transports();
  }
  let endpoint = builder.bind().await.context("绑定 iroh endpoint 失败")?;
  if tokio::time::timeout(OPERATION_TIMEOUT, endpoint.online())
    .await
    .is_err()
  {
    endpoint.close().await;
    anyhow::bail!("连接 relay 超时，请先启动 relay 并检查 --relay-url");
  }
  Ok(endpoint)
}

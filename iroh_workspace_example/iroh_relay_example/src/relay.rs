//! 中继基础设施：按 EndpointId 转发加密包，不处理 echo 应用协议。
use std::net::SocketAddr;

use anyhow::{Context, Result, bail};
use iroh_relay::server::{RelayConfig, Server, ServerConfig};

pub async fn spawn(bind: SocketAddr, metrics: Option<SocketAddr>) -> Result<Server> {
  let mut config = ServerConfig::default();
  config.relay = Some(RelayConfig::new(bind));
  config.metrics_addr = metrics;
  // quic=None：不启动 QAD 地址发现服务；应用的端到端 QUIC 仍然可用。
  Server::spawn(config).await.context("启动 relay 失败")
}

pub async fn run(bind: SocketAddr, metrics: Option<SocketAddr>) -> Result<()> {
  let mut relay = spawn(bind, metrics).await?;
  println!(
    "relay listening: {:?} (HTTP，本地开发模式)",
    relay.http_addr()
  );
  if let Some(addr) = metrics {
    println!("metrics: http://{addr}/metrics");
  }
  println!("按 Ctrl+C 停止 relay");
  let result = tokio::select! {
      result = tokio::signal::ctrl_c() => result.context("监听 Ctrl+C 失败"),
      result = relay.join() => {
          result.context("relay supervisor 任务失败")?.context("relay 服务失败")?;
          bail!("relay 意外退出");
      }
  };
  let shutdown = relay.shutdown().await.context("关闭 relay 失败");
  result.and(shutdown)
}

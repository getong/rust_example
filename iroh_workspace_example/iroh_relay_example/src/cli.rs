//! 命令行：relay 基础设施、QUIC 应用 server/client、一键演示。
use std::net::SocketAddr;

use clap::{Args, Parser, Subcommand};
use iroh::{EndpointId, RelayUrl};

#[derive(Debug, Parser)]
#[command(version, about = "自建 iroh relay + 端到端 QUIC echo")]
pub struct Cli {
  #[command(subcommand)]
  pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
  /// 启动本地 HTTP relay（与应用 QUIC server 是不同角色）
  Relay {
    #[arg(long, default_value = "127.0.0.1:3340")]
    bind: SocketAddr,
    /// 可选 Prometheus 指标监听地址，例如 127.0.0.1:9090
    #[arg(long)]
    metrics: Option<SocketAddr>,
  },
  /// 接收 QUIC 连接并回显数据；打印供 client 使用的 EndpointId
  Server(Network),
  /// 通过 relay 找到应用 server，发送一条消息并校验回显
  Client {
    #[command(flatten)]
    network: Network,
    #[arg(long)]
    server: EndpointId,
    #[arg(long, default_value = "你好，iroh QUIC!")]
    message: String,
  },
  /// 自动启动 relay/server/client，强制经 relay 通信，然后清理资源
  Demo {
    #[arg(long, default_value = "你好，iroh QUIC!")]
    message: String,
  },
}

#[derive(Debug, Clone, Args)]
pub struct Network {
  #[arg(long, default_value = "http://127.0.0.1:3340")]
  pub relay_url: RelayUrl,
  /// 允许尝试 UDP 直连；默认关闭 IP transports，强制经过 relay
  #[arg(long)]
  pub allow_direct: bool,
}

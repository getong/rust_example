//! QUIC 客户端：按公钥与 relay URL 连接，发送数据、校验回显及实际路径。
use anyhow::{Context, Result, ensure};
use iroh::{EndpointAddr, EndpointId};

use crate::{
  cli::Network,
  endpoint::{self, OPERATION_TIMEOUT},
  protocol::{ALPN, MAX_MESSAGE_SIZE},
};

#[derive(Debug)]
pub struct EchoResponse {
  pub bytes: Vec<u8>,
  pub selected_relay: bool,
}

pub async fn run(network: &Network, server: EndpointId, message: &[u8]) -> Result<EchoResponse> {
  ensure!(
    message.len() <= MAX_MESSAGE_SIZE,
    "消息不能超过 {MAX_MESSAGE_SIZE} 字节"
  );
  let endpoint = endpoint::bind(network).await?;
  let result = tokio::time::timeout(OPERATION_TIMEOUT, async {
    let addr = EndpointAddr::new(server).with_relay_url(network.relay_url.clone());
    let connection = endpoint
      .connect(addr, ALPN)
      .await
      .context("建立 QUIC 连接失败")?;
    let (mut send, mut recv) = connection.open_bi().await?;
    send.write_all(message).await?;
    send.finish()?;
    let bytes = recv.read_to_end(MAX_MESSAGE_SIZE).await?;
    ensure!(bytes == message, "echo 响应与请求不一致");
    let paths = connection.paths();
    let selected_relay = paths
      .iter()
      .any(|path| path.is_selected() && path.is_relay());
    for path in paths.iter() {
      println!(
        "path: {:?}, selected={}, relay={}",
        path.remote_addr(),
        path.is_selected(),
        path.is_relay()
      );
    }
    if !network.allow_direct {
      ensure!(selected_relay, "relay-only 模式未检测到选中的 relay 路径");
    }
    println!(
      "echo: {} ({} bytes)",
      String::from_utf8_lossy(&bytes),
      bytes.len()
    );
    connection.close(0u32.into(), b"echo complete");
    Ok(EchoResponse {
      bytes,
      selected_relay,
    })
  })
  .await
  .context("QUIC 请求超时");
  endpoint.close().await;
  result?
}

//! 单进程启动三个独立网络角色，使用真实 socket 与 relay，不使用 mock。
use anyhow::{Context, Result};

use crate::{cli::Network, client, relay, server};

pub async fn run(message: &[u8]) -> Result<()> {
  let relay = relay::spawn("127.0.0.1:0".parse()?, None).await?;
  let result = async {
    let addr = relay.http_addr().context("relay 未监听 HTTP")?;
    let network = Network {
      relay_url: format!("http://{addr}").parse()?,
      allow_direct: false,
    };
    println!("relay: {}", network.relay_url);
    let router = server::spawn(&network).await?;
    println!("server: {}", router.endpoint().id());
    let result = client::run(&network, router.endpoint().id(), message).await;
    let shutdown = router.shutdown().await.context("关闭 server 失败");
    result?;
    shutdown?;
    Ok(())
  }
  .await;
  let shutdown = relay.shutdown().await.context("关闭 relay 失败");
  result.and(shutdown)?;
  println!("demo 完成：已验证经自建 relay 的 QUIC echo，资源已关闭");
  Ok(())
}

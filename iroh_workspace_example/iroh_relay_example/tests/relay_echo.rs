//! 真实 loopback relay 上的 QUIC 集成测试，不依赖公共 relay。
use anyhow::{Context, Result};
use iroh::EndpointAddr;
use iroh_relay_example::{
  cli::Network,
  client, endpoint,
  protocol::{ALPN, MAX_MESSAGE_SIZE},
  relay, server,
};

#[tokio::test]
async fn echo_and_protocol_limits_over_private_relay() -> Result<()> {
  tokio::time::timeout(std::time::Duration::from_secs(90), exercise()).await?
}

async fn exercise() -> Result<()> {
  let relay = relay::spawn("127.0.0.1:0".parse()?, None).await?;
  let network = Network {
    relay_url: format!("http://{}", relay.http_addr().context("HTTP address")?).parse()?,
    allow_direct: false,
  };
  let router = server::spawn(&network).await?;
  let result: Result<()> = async {
    for message in [
      Vec::new(),
      "你好，QUIC 🦀".as_bytes().to_vec(),
      vec![42; MAX_MESSAGE_SIZE],
    ] {
      let response = client::run(&network, router.endpoint().id(), &message).await?;
      assert_eq!(response.bytes, message);
      assert!(
        response.selected_relay,
        "must actually use the private relay"
      );
    }
    let oversized = vec![0; MAX_MESSAGE_SIZE + 1];
    assert!(
      client::run(&network, router.endpoint().id(), &oversized)
        .await
        .is_err()
    );

    // 绕过 client 的本地检查，验证服务端也拒绝超长数据。
    let peer = endpoint::bind(&network).await?;
    let address =
      EndpointAddr::new(router.endpoint().id()).with_relay_url(network.relay_url.clone());
    let result: Result<()> = async {
      assert!(
        peer
          .connect(address.clone(), b"unsupported/alpn")
          .await
          .is_err()
      );
      let conn = peer.connect(address, ALPN).await?;
      let (mut send, mut recv) = conn.open_bi().await?;
      let write = send.write_all(&oversized).await;
      if write.is_ok() {
        let _ = send.finish(); // 服务端可能已经拒绝并关闭连接。
      }
      assert!(recv.read_to_end(MAX_MESSAGE_SIZE + 1).await.is_err());
      Ok(())
    }
    .await;
    peer.close().await;
    result
  }
  .await;
  let server_shutdown = router.shutdown().await;
  let relay_shutdown = relay.shutdown().await;
  result?;
  server_shutdown?;
  relay_shutdown?;
  Ok(())
}

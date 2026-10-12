//! 应用协议：每条连接一个双向 QUIC stream，以 FIN 分隔完整请求/响应。
use iroh::{
  endpoint::Connection,
  protocol::{AcceptError, ProtocolHandler},
};

use crate::endpoint::OPERATION_TIMEOUT;

pub const ALPN: &[u8] = b"iroh-relay-demo/echo/1";
pub const MAX_MESSAGE_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct Echo;

impl ProtocolHandler for Echo {
  async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
    let result = tokio::time::timeout(OPERATION_TIMEOUT, async {
      let (mut send, mut recv) = connection.accept_bi().await?;
      let request = recv
        .read_to_end(MAX_MESSAGE_SIZE)
        .await
        .map_err(std::io::Error::other)?;
      tracing::info!(peer = %connection.remote_id(), bytes = request.len(), "echo request");
      send
        .write_all(&request)
        .await
        .map_err(std::io::Error::other)?;
      send.finish()?;
      // 不立即关闭连接，否则响应可能尚未送达 client。
      connection.closed().await;
      Ok::<_, AcceptError>(())
    })
    .await;
    match result {
      Ok(result) => {
        if result.is_err() {
          connection.close(1u32.into(), b"invalid echo request");
        }
        result
      }
      Err(_) => {
        connection.close(2u32.into(), b"echo timeout");
        Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "echo timeout").into())
      }
    }
  }
}

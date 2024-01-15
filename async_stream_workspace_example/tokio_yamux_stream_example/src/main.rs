use anyhow::Result;
use futures::prelude::*;
use tokio::net::TcpListener;
use tokio_util::{
  codec::{Framed, LinesCodec},
  compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt},
};
use tracing::info;
use yamux::{Config, Connection, Mode};

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt::init();
  let addr = "0.0.0.0:8080";
  let listener = TcpListener::bind(addr).await?;
  info!("Listening on: {:?}", addr);
  loop {
    let (stream, addr) = listener.accept().await?;
    info!("Accepted: {:?}", addr);
    let config = Config::default();
    // 使用 compat() 方法把 tokio AsyncRead/AsyncWrite 转换成 futures 对应的 trait
    let mut conn = Connection::new(stream.compat(), config, Mode::Server);

    tokio::spawn(async move {
      stream::poll_fn(move |cx| {
        conn
          .poll_new_outbound(cx)
          .map(|result| result.map(Some).transpose())
      })
      .try_for_each_concurrent(None, move |s| async move {
        // 使用 compat() 方法把 futures AsyncRead/AsyncWrite 转换成 tokio 对应的 trait
        let mut framed = Framed::new(s.compat(), LinesCodec::new());
        while let Some(Ok(line)) = framed.next().await {
          println!("Got: {}", line);
          framed
            .send(format!("Hello! I got '{}'", line))
            .await
            .unwrap();
        }

        Ok(())
      })
    });
  }
}

// copy from https://github.com/tyrchen/geektime-rust
// modified with chatpgpt
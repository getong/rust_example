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
  let mut config = Config::default();
  config.set_split_send_size(4 * 1024);
  loop {
    let (stream, addr) = listener.accept().await?;
    info!("Accepted: {:?}", addr);
    let config = config.clone();

    tokio::spawn(async move {
      // 使用 compat() 方法把 tokio AsyncRead/AsyncWrite 转换成 futures 对应的 trait
      let mut conn = Connection::new(stream.compat(), config, Mode::Server);
      loop {
        match stream::poll_fn(|cx| conn.poll_next_inbound(cx))
          .next()
          .await
        {
          Some(Ok(stream)) => {
            let mut framed = Framed::new(stream.compat(), LinesCodec::new());
            while let Some(Ok(line)) = framed.next().await {
              println!("Got: {}", line);
              framed
                .send(format!("Hello! I got '{}'", line))
                .await
                .unwrap();
            }
            break;
          }
          Some(Err(e)) => {
            println!("e :{:?}", e);
            break;
          }
          None => {
            // println!("none")
            break;
          }
        }
      }
    });
  }
}

// copy from https://github.com/tyrchen/geektime-rust
// modified with chatpgpt

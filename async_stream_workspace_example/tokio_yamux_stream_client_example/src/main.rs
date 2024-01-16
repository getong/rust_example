use anyhow::Result;
use futures::prelude::*;
use tokio::net::TcpStream;
use tokio_util::{
  codec::{Framed, LinesCodec},
  compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt},
};
use tracing::info;
use yamux::{Config, Connection, Mode};

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt::init();
  let stream = TcpStream::connect("127.0.01:8080").await?;
  info!("Connected to server");
  let mut config = Config::default();
  config.set_split_send_size(4 * 1024);
  let mut conn = Connection::new(stream.compat(), config, Mode::Client);

  // poll 所有 stream 下的数据
  let stream = future::poll_fn(move |cx| conn.poll_new_outbound(cx))
    .await
    .unwrap();

  let stream = stream.compat();
  info!("Started a new stream");
  let mut framed = Framed::new(stream, LinesCodec::new());
  // framed
  //   .send("Hello, this is Tyr!".to_string())
  //   .await
  //   .unwrap();
  if let Err(err) = framed.send("Hello, this is Tyr!".to_string()).await {
    eprintln!("Error sending message: {:?}", err);
    // Handle the error appropriately, e.g., return Err(err) or take other actions
  }
  if let Some(Ok(line)) = framed.next().await {
    println!("Got: {}", line);
  }

  Ok(())
}

// copy from https://github.com/tyrchen/geektime-rust
// modified with chatpgpt

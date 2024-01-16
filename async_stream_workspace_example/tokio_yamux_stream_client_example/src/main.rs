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
  let stream = future::poll_fn(|cx| conn.poll_new_outbound(cx))
    .await
    .unwrap();

  tokio::spawn(noop_server(stream::poll_fn(move |cx| {
    conn.poll_next_inbound(cx)
  })));

  let stream = stream.compat();
  info!("Started a new stream");
  let mut framed = Framed::new(stream, LinesCodec::new());
  loop {
    tokio::select! {
      result = framed.send("Hello, this is Tyr!".to_string()).fuse() => {
        // Handle the result of the send operation
        if let Err(err) = result {
          eprintln!("Error sending message: {:?}", err);
          // Optionally: return Err(err) or take other actions
        }
      },
    }

    tokio::select! {
      response = framed.next().fuse() => {
        // Handle the received response
        if let Some(Ok(line)) = response {
          println!("Got: {}", line);
        }
        // Optionally: Handle other cases if needed
      },
    }
  }
}

/// For each incoming stream, do nothing.
pub async fn noop_server(c: impl Stream<Item = Result<yamux::Stream, yamux::ConnectionError>>) {
  c.for_each(|maybe_stream| {
    drop(maybe_stream);
    future::ready(())
  })
  .await;
}
// copy from https://github.com/tyrchen/geektime-rust
// modified with chatpgpt

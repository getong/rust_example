use anyhow::Result;
use futures::prelude::*;
use futures::Stream as FutureStream;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::{
  codec::{Framed, LinesCodec},
  compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt},
};
use tracing::info;
use yamux::{Config, Connection, Mode, Stream};

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt::init();
  let addr = "0.0.0.0:8080";
  let listener = TcpListener::bind(addr).await?;
  info!("Listening on: {:?}", addr);
  let mut config = Config::default();
  config.set_split_send_size(4 * 1024);
  run_server(listener, config).await?;
  Ok(())
}

async fn run_server(listener: TcpListener, config: Config) -> Result<()> {
  loop {
    let (stream, addr) = listener.accept().await?;
    info!("Accepted: {:?}", addr);

    tokio::spawn(handle_connection(stream, config.clone()));
  }
}

async fn handle_connection(stream: TcpStream, config: Config) {
  let mut conn = Connection::new(stream.compat(), config, Mode::Server);

  let mut server = stream::poll_fn(move |cx| conn.poll_next_inbound(cx));
  match server.next().await {
    Some(Ok(stream)) => {
      tokio::spawn(noop_server(server));
      process_client(stream).await;
    }
    Some(Err(e)) => {
      handle_error(e);
    }
    None => {
      // Handle None case if needed
    }
  }
}

async fn process_client(stream: Stream) {
  let mut framed = Framed::new(stream.compat(), LinesCodec::new());

  while let Some(Ok(line)) = framed.next().await {
    println!("Got: {}", line);
    framed
      .send(format!("Hello! I got '{}'", line))
      .await
      .unwrap();
  }
}

fn handle_error(error: yamux::ConnectionError) {
  println!("Error: {:?}", error);
  // Handle the error as needed
}

/// For each incoming stream, do nothing.
pub async fn noop_server(
  c: impl FutureStream<Item = Result<yamux::Stream, yamux::ConnectionError>>,
) {
  c.for_each(|maybe_stream| {
    drop(maybe_stream);
    future::ready(())
  })
  .await;
}

// copy from https://github.com/tyrchen/geektime-rust
// modified with chatpgpt

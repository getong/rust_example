use anyhow::Result;
use futures::prelude::*;
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
  let config = Config::default();
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

  loop {
    match stream::poll_fn(|cx| conn.poll_next_inbound(cx))
      .next()
      .await
    {
      Some(Ok(stream)) => {
        process_client(stream).await;
        break;
      }
      Some(Err(e)) => {
        handle_error(e);
        break;
      }
      None => {
        // Handle None case if needed
        break;
      }
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

// copy from https://github.com/tyrchen/geektime-rust
// modified with chatpgpt

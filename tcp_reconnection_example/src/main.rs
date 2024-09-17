use derive_deref::{Deref, DerefMut};
use std::error::Error;
use std::future::Future;
use std::io;
use std::pin::Pin;
use stubborn_io::config::DurationIterator;
use stubborn_io::tokio::{StubbornIo, UnderlyingIo};
use stubborn_io::ReconnectOptions;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, Duration};

#[derive(Deref, DerefMut)]
struct DurableTCPStream(TcpStream);

impl AsyncWrite for DurableTCPStream {
  fn poll_write(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &[u8],
  ) -> std::task::Poll<Result<usize, io::Error>> {
    <TcpStream as AsyncWrite>::poll_write(Pin::new(&mut self.0), cx, buf)
  }

  fn poll_flush(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), io::Error>> {
    <TcpStream as AsyncWrite>::poll_flush(Pin::new(&mut self.0), cx)
  }

  fn poll_shutdown(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), io::Error>> {
    <TcpStream as AsyncWrite>::poll_shutdown(Pin::new(&mut self.0), cx)
  }

  fn poll_write_vectored(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    bufs: &[io::IoSlice<'_>],
  ) -> std::task::Poll<Result<usize, io::Error>> {
    <TcpStream as AsyncWrite>::poll_write_vectored(Pin::new(&mut self.0), cx, bufs)
  }

  fn is_write_vectored(&self) -> bool {
    <TcpStream as AsyncWrite>::is_write_vectored(&self.0)
  }
}

impl UnderlyingIo<String> for DurableTCPStream {
  fn establish(addr: String) -> Pin<Box<dyn Future<Output = io::Result<Self>> + Send>> {
    Box::pin(async move {
      let parts: Vec<&str> = addr.split('/').collect();
      println!("connecting to {}", parts[0]);
      let mut stream = TcpStream::connect(parts[0]).await?;

      if parts.len() > 1 {
        // hello message was specified, use it
        println!("sending login: {}", parts[1]);
        stream.write_all(format!("{}\n", parts[1]).as_ref()).await?;
      }
      Ok(DurableTCPStream(stream))
    })
  }
}

type StubbornTCP = StubbornIo<DurableTCPStream, String>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Create and configure the StubbornIo instance
  let mut stream = StubbornTCP::connect_with_options(
    String::from("localhost:9999/logincmd"),
    ReconnectOptions::new()
      .with_exit_if_first_connect_fails(false)
      .with_retries_generator(get_our_standard_reconnect_strategy),
  )
  .await?;

  loop {
    println!("sending msg");
    match stream.write_all(b"hello world!\n").await {
      Ok(_) => (),
      Err(e) => {
        println!("Error writing to stream: {}", e);
      }
    }
    sleep(Duration::from_secs(1)).await;
  }
}

fn get_our_standard_reconnect_strategy() -> DurationIterator {
  let initial_attempts = vec![
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(20),
    Duration::from_secs(30),
    Duration::from_secs(40),
    Duration::from_secs(50),
    Duration::from_secs(60),
  ];

  let repeat = std::iter::repeat(Duration::from_secs(60));

  let forever_iterator = initial_attempts.into_iter().chain(repeat);

  Box::new(forever_iterator)
}

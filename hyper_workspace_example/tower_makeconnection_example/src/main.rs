use std::{
  net::SocketAddr,
  pin::Pin,
  task::{Context, Poll},
};

use futures::Future;
use tokio::{io::AsyncWriteExt, net::TcpStream};
use tower_service::Service;

struct MyMakeConnection;

impl Service<SocketAddr> for MyMakeConnection {
  type Response = TcpStream;
  type Error = std::io::Error;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn call(&mut self, target: SocketAddr) -> Self::Future {
    Box::pin(async move {
      let stream = TcpStream::connect(target).await;
      stream
    })
  }
}

// nc -l 8080
#[tokio::main]
async fn main() {
  let mut maker = MyMakeConnection;
  let addr = "127.0.0.1:8080".parse().unwrap();

  match maker.call(addr).await {
    Ok(mut connection) => {
      println!("Successfully connected to {:?}", connection);
      // Use the connection
      _ = connection.write_all(b"hello world").await;
    }
    Err(e) => println!("Failed to connect: {}", e),
  }
}

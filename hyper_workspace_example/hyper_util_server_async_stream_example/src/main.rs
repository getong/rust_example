use async_stream::try_stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use http_body_util::Full;
use hyper::{service::service_fn, Request, Response};
use hyper_util::{rt::TokioExecutor, rt::TokioIo, server::conn::auto};
use std::io;
use std::net::SocketAddr;
use tokio::{net::TcpListener, net::TcpStream};

#[derive(Debug, thiserror::Error)]
enum Error {
  #[error("IO error: {0}")]
  Io(#[from] std::io::Error),
}

static INDEX1: &[u8] = b"The 1st service!\n";

async fn index1(_: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, hyper::Error> {
  Ok(Response::new(Full::new(Bytes::from(INDEX1))))
}

async fn bind_and_accept(
  addr: SocketAddr,
) -> impl Stream<Item = io::Result<(TcpStream, SocketAddr)>> {
  try_stream! {
    let listener = TcpListener::bind(addr).await?;

    loop {
      yield listener.accept().await?;
    }
  }
}

// curl http://localhost:3000
#[tokio::main]
async fn main() -> Result<(), Error> {
  let addr = "127.0.0.1:3000".parse().unwrap();
  let stream = bind_and_accept(addr).await;
  let mut stream = std::pin::pin!(stream);
  while let Some(Ok((socket, _perr_addr))) = stream.next().await {
    let io = TokioIo::new(socket);

    tokio::task::spawn(async move {
      if let Err(err) = auto::Builder::new(TokioExecutor::new())
        .serve_connection(io, service_fn(index1))
        .await
      {
        println!("Error serving connection: {:?}", err);
      }
    });
  }
  Ok(())
}

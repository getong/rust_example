use std::env;

use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::Request;
use hyper_util::rt::TokioIo;
use tokio::{
  io::{self, AsyncWriteExt as _},
  net::TcpStream,
};

// A simple type alias so as to DRY.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> Result<()> {
  pretty_env_logger::init();

  // Some simple CLI args requirements...
  let url = match env::args().nth(1) {
    Some(url) => url,
    None => {
      println!("Usage: client <url>");
      return Ok(());
    }
  };

  // HTTPS requires picking a TLS implementation, so give a better
  // warning if the user tries to request an 'https' URL.
  let url = url.parse::<hyper::Uri>().unwrap();
  if url.scheme_str() != Some("http") {
    println!("This example only works with 'http' URLs.");
    return Ok(());
  }

  fetch_url(url).await
}

async fn fetch_url(url: hyper::Uri) -> Result<()> {
  let host = url.host().expect("uri has no host");
  let port = url.port_u16().unwrap_or(80);
  let addr = format!("{}:{}", host, port);
  let stream = TcpStream::connect(addr).await?;
  let io = TokioIo::new(stream);

  let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
  tokio::task::spawn(async move {
    if let Err(err) = conn.await {
      println!("Connection failed: {:?}", err);
    }
  });

  let authority = url.authority().unwrap().clone();

  let req = Request::builder()
    .uri(url)
    .header(hyper::header::HOST, authority.as_str())
    .body(Empty::<Bytes>::new())?;

  let mut res = sender.send_request(req).await?;

  println!("Response: {}", res.status());
  println!("Headers: {:#?}\n", res.headers());

  // Stream the body, writing each chunk to stdout as we get it
  // (instead of buffering and printing at the end).
  while let Some(next) = res.frame().await {
    // let frame = next?;
    if let Ok(ref frame) = next {
      if let Some(chunk) = frame.data_ref() {
        io::stdout().write_all(&chunk).await?;
      }
    }
  }

  println!("\n\nDone!");

  Ok(())
}

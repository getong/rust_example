use http_body_util::Empty;
use hyper::Request;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use std::env;
use tower_service::Service;

// cargo run -- http://www.baidu.com
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let url = match env::args().nth(1) {
    Some(url) => url,
    None => {
      eprintln!("Usage: client <url>");
      return Ok(());
    }
  };

  // HTTPS requires picking a TLS implementation, so give a better
  // warning if the user tries to request an 'https' URL.
  let url = url.parse::<hyper::Uri>()?;
  if url.scheme_str() != Some("http") {
    eprintln!("This example only works with 'http' URLs.");
    return Ok(());
  }

  let mut client =
    Client::builder(hyper_util::rt::TokioExecutor::new()).build(HttpConnector::new());

  let req = Request::builder()
    .uri(url)
    .body(Empty::<bytes::Bytes>::new())?;

  let resp = client.request(req.clone()).await?;
  eprintln!("{:?} {:?}", resp.version(), resp.status());
  eprintln!("{:#?}", resp.headers());

  let result = client.call(req).await;
  eprintln!("result : {:?}", result);

  Ok(())
}

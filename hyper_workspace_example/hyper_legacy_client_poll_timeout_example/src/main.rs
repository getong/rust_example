use std::time::Duration;

use http_body_util::{BodyExt, Empty};
use hyper::Request;
use hyper_util::{
  client::legacy::Client,
  rt::{TokioExecutor, TokioTimer},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let client = Client::builder(TokioExecutor::new())
    .pool_idle_timeout(Duration::from_secs(30))
    .pool_timer(TokioTimer::new())
    .build_http();

  let url = "http://www.baidu.com".parse::<hyper::Uri>()?;
  let req = Request::builder()
    .uri(url)
    .body(Empty::<bytes::Bytes>::new())?;

  let resp = client.request(req).await?;

  eprintln!("version: {:?} status: {:?}", resp.version(), resp.status());
  eprintln!("headers: {:#?}", resp.headers());

  let body = resp.collect().await?.to_bytes().to_vec();
  match std::str::from_utf8(&body) {
    Ok(s) => eprintln!("body: {:#?}", s),
    Err(_) => panic!("Invalid UTF-8 data"),
  };

  Ok(())
}

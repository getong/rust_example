use std::{
  error::Error,
  task::{Context, Poll},
};

use futures_util::future::{BoxFuture, FutureExt};
use http_body_util::Empty;
use hyper::{Request, Uri};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use tower_service::Service;

// Define your custom connector type.
#[derive(Clone)]
struct MyConnector {
  http: HttpConnector,
}

impl MyConnector {
  fn new() -> Self {
    MyConnector {
      http: HttpConnector::new(),
    }
  }
}

impl Service<Uri> for MyConnector {
  type Response = <HttpConnector as Service<Uri>>::Response;
  type Error = <HttpConnector as Service<Uri>>::Error;
  type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

  fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    self.http.poll_ready(cx)
  }

  fn call(&mut self, uri: Uri) -> Self::Future {
    // Here you can add your custom connection logic.
    // For demonstration, we're just using the HttpConnector directly.
    self.http.call(uri).boxed()
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let my_connector = MyConnector::new();
  let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build(my_connector);

  let url = "http://httpbin.org/ip";
  let req = Request::builder()
    .uri(url)
    .body(Empty::<bytes::Bytes>::new())?;

  let response = client.request(req).await?;

  println!("Response: {}", response.status());
  Ok(())
}

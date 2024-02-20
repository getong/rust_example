use bytes::Bytes;
use futures_util::{
  future::{self, BoxFuture},
  FutureExt,
};
use http_body_util::{BodyExt, Full};
use hyper::{Error, Response, Uri};
use std::task::{Context, Poll};
use tower_service::Service;

struct UriService;

impl Service<Uri> for UriService {
  type Response = Response<Full<Bytes>>;
  type Error = Error;
  type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

  // Note the change to &mut self here
  fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  // Note the change to &mut self here
  fn call(&mut self, uri: Uri) -> Self::Future {
    let path = uri.path().to_string();

    let response = match path.as_str() {
      "/hello" => Response::new(Full::from(Bytes::from("Hello, world!"))),
      "/" => Response::new(Full::from(Bytes::from("Welcome to the root page!"))),
      _ => Response::builder()
        .status(404)
        .body(Full::from(Bytes::from("Not Found")))
        .unwrap(),
    };

    future::ready(Ok(response)).boxed()
  }
}

#[tokio::main]
async fn main() {
  let mut service = UriService; // Note: Service instances might need to be mutable
  let uri = Uri::from_static("/hello");

  match service.call(uri).await {
    Ok(response) => {
      let body = response
        .into_body()
        .frame()
        .await
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
      println!("Response: {}", std::str::from_utf8(&body).unwrap());
    }
    Err(e) => eprintln!("Error calling service: {}", e),
  }

  let uri = Uri::from_static("/other");
  match service.call(uri).await {
    Ok(response) => {
      let body = response
        .into_body()
        .frame()
        .await
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
      println!("Response: {}", std::str::from_utf8(&body).unwrap());
    }
    Err(e) => eprintln!("Error calling service: {}", e),
  }
}

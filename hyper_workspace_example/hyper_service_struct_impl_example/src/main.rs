use std::{
  future::Future,
  net::SocketAddr,
  pin::Pin,
  sync::{Arc, Mutex},
};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{body::Incoming, service::Service, Request, Response};
use hyper_util::{
  rt::{TokioExecutor, TokioIo},
  server::conn::auto,
};
use tokio::net::TcpListener;

type Counter = i32;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

  let listener = TcpListener::bind(addr).await?;
  println!("Listening on http://{}", addr);

  let svc = Svc {
    counter: Arc::new(Mutex::new(0)),
  };

  loop {
    let (stream, _) = listener.accept().await?;
    let io = TokioIo::new(stream);
    let svc_clone = svc.clone();
    tokio::task::spawn(async move {
      if let Err(err) = auto::Builder::new(TokioExecutor::new())
        .serve_connection(io, svc_clone)
        .await
      {
        println!("Failed to serve connection: {:?}", err);
      }
    });
  }
}

#[derive(Debug, Clone)]
struct Svc {
  counter: Arc<Mutex<Counter>>,
}

impl Service<Request<Incoming>> for Svc {
  type Response = Response<Full<Bytes>>;
  type Error = hyper::Error;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn call(&self, req: Request<Incoming>) -> Self::Future {
    fn mk_response(s: String) -> Result<Response<Full<Bytes>>, hyper::Error> {
      Ok(Response::builder().body(Full::new(Bytes::from(s))).unwrap())
    }

    if req.uri().path() != "/favicon.ico" {
      *self.counter.lock().expect("lock poisoned") += 1;
    }

    let res = match req.uri().path() {
      "/" => mk_response(format!("home! counter = {:?}", self.counter)),
      "/posts" => mk_response(format!("posts, of course! counter = {:?}", self.counter)),
      "/authors" => mk_response(format!(
        "authors extraordinare! counter = {:?}",
        self.counter
      )),
      // Return the 404 Not Found for other routes, and don't increment counter.
      _ => return Box::pin(async { mk_response("oh no! not found".into()) }),
    };

    Box::pin(async { res })
  }
}

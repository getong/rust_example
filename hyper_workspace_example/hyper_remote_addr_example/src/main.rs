use bytes::Bytes;
use http_body_util::Full;
use hyper::{body::Incoming, server::conn::http1, service::Service, Request, Response};
use hyper_util::{client::legacy::connect::HttpInfo, rt::TokioIo};
use std::{future::Future, net::SocketAddr, pin::Pin};
use tokio::net::TcpListener;

// curl http://127.0.0.1:3000
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

  let listener = TcpListener::bind(addr).await?;
  println!("Listening on http://{}", addr);

  let svc = Svc;

  loop {
    let (stream, _) = listener.accept().await?;
    let io = TokioIo::new(stream);
    let svc_clone = svc.clone();
    tokio::task::spawn(async move {
      if let Err(err) = http1::Builder::new().serve_connection(io, svc_clone).await {
        println!("Failed to serve connection: {:?}", err);
      }
    });
  }
}

#[derive(Debug, Clone)]
struct Svc;

impl Service<Request<Incoming>> for Svc {
  type Response = Response<Full<Bytes>>;
  type Error = hyper::Error;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn call(&self, req: Request<Incoming>) -> Self::Future {
    let (_parts, body) = req.into_parts();
    let resp = Response::new(body);

    let remote_addr = resp
      .extensions()
      .get::<HttpInfo>()
      .map(|info| info.remote_addr())
      .unwrap();

    let s = format!("{}", remote_addr);

    let res = Ok(Response::builder().body(Full::new(Bytes::from(s))).unwrap());

    Box::pin(async { res })
  }
}

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::TcpListener;

async fn handle_request(
  req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
  let peer_addr = req.extensions().get::<SocketAddr>().cloned();

  let response_body = match peer_addr {
    Some(addr) => format!("Your address: {}", addr),
    None => "Hello, world!".to_string(),
  };

  Ok(Response::new(
    Full::new(response_body.into())
      .map_err(|never| match never {})
      .boxed(),
  ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

  let listener = TcpListener::bind(addr).await?;
  println!("Listening on http://{}", addr);
  loop {
    let (stream, peer_addr) = listener.accept().await?;
    let io = TokioIo::new(stream);

    tokio::task::spawn(async move {
      if let Err(err) = http1::Builder::new()
        .serve_connection(
          io,
          service_fn(move |mut req| {
            req.extensions_mut().insert(peer_addr);
            handle_request(req)
          }),
        )
        .await
      {
        println!("Error serving connection: {:?}", err);
      }
    });
  }
}

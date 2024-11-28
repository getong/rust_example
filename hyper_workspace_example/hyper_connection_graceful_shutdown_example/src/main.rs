use std::{
  net::SocketAddr,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{service::service_fn, Request, Response};
use hyper_util::{
  rt::{TokioExecutor, TokioIo},
  server::conn::auto,
};
use tokio::{net::TcpListener, sync::watch};

static INDEX1: &[u8] = b"The 1st service!\n";

async fn index1(_: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, hyper::Error> {
  Ok(Response::new(Full::new(Bytes::from(INDEX1))))
}

// curl http://localhost:1337
#[tokio::main]
async fn main() {
  let running = Arc::new(AtomicBool::new(true));
  let r = running.clone();

  ctrlc::set_handler(move || {
    r.store(false, Ordering::SeqCst);
  })
  .expect("Error setting Ctrl-C handler");

  let addr: SocketAddr = ([127, 0, 0, 1], 1337).into();

  let (tx, mut rx) = watch::channel(false);

  let listener = TcpListener::bind(addr).await.unwrap();
  tokio::spawn(async move {
    loop {
      tokio::select! {
        res = listener.accept() => {
          let (stream, _) = res.expect("Failed to accept");
          let io = TokioIo::new(stream);

          let binding = auto::Builder::new(TokioExecutor::new());
          tokio::task::spawn(async move {
            let connection = binding.serve_connection(io, service_fn(index1));
            let mut connection = std::pin::pin!(connection);
            connection.as_mut().graceful_shutdown();
            if let Err(err) = connection.await {
              println!("Error serving connection: {:?}", err);
              return;
            }
          });
        }
        _ = rx.changed() => {
          break;
        }
      }
    }
  });

  while running.load(Ordering::SeqCst) {}

  _ = tx.send(true);
  println!("hyper server exit...");
}

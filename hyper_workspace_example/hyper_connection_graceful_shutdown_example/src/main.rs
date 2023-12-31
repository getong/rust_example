use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::watch;

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

          let mut rx = rx.clone();
          tokio::task::spawn(async move {
            let connection = binding.serve_connection(io, service_fn(index1));
            tokio::pin!(connection);
            tokio::select! {
              res = &mut connection => {
                if let Err(err) = res {
                  println!("Error serving connection: {:?}", err);
                  return;
                }
              }
              // Continue polling the connection after enabling graceful shutdown.
              _ = rx.changed() => {
                connection.graceful_shutdown();
              }
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

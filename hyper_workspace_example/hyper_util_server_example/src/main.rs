use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto;
use std::net::SocketAddr;
use tokio::net::TcpListener;

static INDEX1: &[u8] = b"The 1st service!\n";

async fn index1(_: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, hyper::Error> {
    Ok(Response::new(Full::new(Bytes::from(INDEX1))))
}

// curl http://localhost:1337

#[tokio::main]
async fn main() {
    let addr1: SocketAddr = ([127, 0, 0, 1], 1337).into();

    let listener = TcpListener::bind(addr1).await.unwrap();
    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = auto::Builder::new(TokioExecutor::new())
                .serve_connection(io, service_fn(index1))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

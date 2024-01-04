use axum::{body::Body, extract::Path, http::Request, routing::get, Router};
use http_body_util::BodyExt as _;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use std::net::{IpAddr, Ipv4Addr};
use tracing::{info_span, Instrument};
use turmoil::{net, Builder};

fn main() {
  if std::env::var("RUST_LOG").is_err() {
    std::env::set_var("RUST_LOG", "info");
  }

  tracing_subscriber::fmt::init();

  let addr = (IpAddr::from(Ipv4Addr::UNSPECIFIED), 9999);

  let mut sim = Builder::new().build();

  let router = Router::new().route(
    "/greet/:name",
    get(|Path(name): Path<String>| async move { format!("Hello {name}!") }),
  );

  sim.host("server", move || {
    let router = router.clone();
    async move {
      let listener = net::TcpListener::bind(addr).await?;
      loop {
        let (tcp_stream, _remote_addr) = listener.accept().await?;
        let tcp_stream = hyper_util::rt::TokioIo::new(tcp_stream);

        let hyper_service = hyper_util::service::TowerToHyperService::new(router.clone());

        let result =
          hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
            .serve_connection_with_upgrades(tcp_stream, hyper_service)
            .await;
        if result.is_err() {
          // This error only appears when the client doesn't send a request and
          // terminate the connection.
          //
          // If client sends one request then terminate connection whenever, it doesn't
          // appear.
          break;
        }
      }

      Ok(())
    }
    .instrument(info_span!("server"))
  });

  sim.client(
    "client",
    async move {
      let client = Client::builder(TokioExecutor::new()).build(connector::connector());

      let mut request = Request::new(Body::empty());
      *request.uri_mut() = hyper::Uri::from_static("http://server:9999/greet/foo");
      let res = client.request(request).await?;

      let (parts, body) = res.into_parts();
      let body = body.collect().await?.to_bytes();
      let res = hyper::Response::from_parts(parts, body);

      tracing::info!("Got response: {:?}", res);

      Ok(())
    }
    .instrument(info_span!("client")),
  );

  sim.run().unwrap();
}

mod connector {
  use hyper::Uri;
  use std::{future::Future, io::Error, pin::Pin};
  use tokio::io::{AsyncRead, AsyncWrite};
  use tower::Service;
  use turmoil::net::TcpStream;

  type Fut = Pin<Box<dyn Future<Output = Result<TurmoilConnection, Error>> + Send>>;

  pub fn connector(
  ) -> impl Service<Uri, Response = TurmoilConnection, Error = Error, Future = Fut> + Clone {
    tower::service_fn(|uri: Uri| {
      Box::pin(async move {
        let conn = TcpStream::connect(uri.authority().unwrap().as_str()).await?;
        Ok::<_, Error>(TurmoilConnection(conn))
      }) as Fut
    })
  }

  pub struct TurmoilConnection(turmoil::net::TcpStream);

  impl hyper::rt::Read for TurmoilConnection {
    fn poll_read(
      self: Pin<&mut Self>,
      cx: &mut std::task::Context<'_>,
      mut buf: hyper::rt::ReadBufCursor<'_>,
    ) -> std::task::Poll<Result<(), Error>> {
      let turmoil_connection = Pin::get_mut(self);
      unsafe {
        let mut tbuf = tokio::io::ReadBuf::uninit(buf.as_mut());

        let result = Pin::new(&mut turmoil_connection.0).poll_read(cx, &mut tbuf);
        match result {
          std::task::Poll::Ready(Ok(())) => {
            let n = tbuf.filled().len();
            buf.advance(n);
            std::task::Poll::Ready(Ok(()))
          }
          other => other,
        }
      }
    }
  }

  impl hyper::rt::Write for TurmoilConnection {
    fn poll_write(
      self: Pin<&mut Self>,
      cx: &mut std::task::Context<'_>,
      buf: &[u8],
    ) -> std::task::Poll<Result<usize, Error>> {
      let turmoil_connection = Pin::get_mut(self);
      Pin::new(&mut turmoil_connection.0).poll_write(cx, buf)
    }

    fn poll_flush(
      self: Pin<&mut Self>,
      cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Error>> {
      let turmoil_connection = Pin::get_mut(self);
      Pin::new(&mut turmoil_connection.0).poll_flush(cx)
    }

    fn poll_shutdown(
      self: Pin<&mut Self>,
      cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Error>> {
      let turmoil_connection = Pin::get_mut(self);
      Pin::new(&mut turmoil_connection.0).poll_shutdown(cx)
    }
  }

  impl hyper_util::client::legacy::connect::Connection for TurmoilConnection {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
      hyper_util::client::legacy::connect::Connected::new()
    }
  }
}

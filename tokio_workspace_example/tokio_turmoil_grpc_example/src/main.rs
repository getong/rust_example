use std::net::{IpAddr, Ipv4Addr};
use tonic::transport::Endpoint;
use tonic::Status;
use tonic::{Request, Response};
use tracing::{info_span, Instrument};
use turmoil::{net, Builder};

#[allow(non_snake_case)]
mod proto {
  tonic::include_proto!("helloworld");
}

use proto::greeter_server::{Greeter, GreeterServer};
use proto::{HelloReply, HelloRequest};

use crate::proto::greeter_client::GreeterClient;

fn main() {
  if std::env::var("RUST_LOG").is_err() {
    std::env::set_var("RUST_LOG", "info");
  }

  tracing_subscriber::fmt::init();

  let addr = (IpAddr::from(Ipv4Addr::UNSPECIFIED), 9999);

  let mut sim = Builder::new().build();

  let greeter = GreeterServer::new(MyGreeter {});

  sim.host("server", move || {
    let greeter = greeter.clone();
    async move {
      let listener = net::TcpListener::bind(addr).await?;
      loop {
        let (tcp_stream, _remote_addr) = listener.accept().await?;
        let tcp_stream = hyper_util::rt::TokioIo::new(tcp_stream);

        let hyper_service = hyper_util::service::TowerToHyperService::new(greeter);

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
      let ch = Endpoint::new("http://server:9999")?
        .connect_with_connector(connector::connector())
        .await?;
      let mut greeter_client = GreeterClient::new(ch);

      let request = Request::new(HelloRequest { name: "foo".into() });
      let res = greeter_client.say_hello(request).await?;

      tracing::info!("Got response: {:?}", res);

      Ok(())
    }
    .instrument(info_span!("client")),
  );

  sim.run().unwrap();
}

#[derive(Default)]
pub struct MyGreeter {}

#[tonic::async_trait]
impl Greeter for MyGreeter {
  async fn say_hello(
    &self,
    request: Request<HelloRequest>,
  ) -> Result<Response<HelloReply>, Status> {
    let reply = HelloReply {
      message: format!("Hello {}!", request.into_inner().name),
    };
    Ok(Response::new(reply))
  }
}

mod connector {
  use hyper::Uri;
  use pin_project_lite::pin_project;
  use std::{future::Future, io::Error, pin::Pin};
  use tokio::io::AsyncWrite;
  use tower::Service;
  use turmoil::net::TcpStream;

  type Fut = Pin<Box<dyn Future<Output = Result<TurmoilConnection, Error>> + Send>>;

  pub fn connector(
  ) -> impl Service<Uri, Response = TurmoilConnection, Error = Error, Future = Fut> + Clone {
    tower::service_fn(|uri: Uri| {
      Box::pin(async move {
        let conn = TcpStream::connect(uri.authority().unwrap().as_str()).await?;
        Ok::<_, Error>(TurmoilConnection { fut: conn })
      }) as Fut
    })
  }

  pin_project! {
    pub struct TurmoilConnection{
      #[pin]
      fut: turmoil::net::TcpStream
    }
  }

  impl hyper::rt::Read for TurmoilConnection {
    fn poll_read(
      self: Pin<&mut Self>,
      cx: &mut std::task::Context<'_>,
      mut buf: hyper::rt::ReadBufCursor<'_>,
    ) -> std::task::Poll<Result<(), Error>> {
      let n = unsafe {
        let mut tbuf = tokio::io::ReadBuf::uninit(buf.as_mut());
        let result = tokio::io::AsyncRead::poll_read(self.project().fut, cx, &mut tbuf);
        match result {
          std::task::Poll::Ready(Ok(())) => tbuf.filled().len(),
          other => return other,
        }
      };

      unsafe {
        buf.advance(n);
      }
      std::task::Poll::Ready(Ok(()))
    }
  }

  impl hyper::rt::Write for TurmoilConnection {
    fn poll_write(
      mut self: Pin<&mut Self>,
      cx: &mut std::task::Context<'_>,
      buf: &[u8],
    ) -> std::task::Poll<Result<usize, Error>> {
      Pin::new(&mut self.fut).poll_write(cx, buf)
    }

    fn poll_flush(
      mut self: Pin<&mut Self>,
      cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Error>> {
      Pin::new(&mut self.fut).poll_flush(cx)
    }

    fn poll_shutdown(
      mut self: Pin<&mut Self>,
      cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Error>> {
      Pin::new(&mut self.fut).poll_shutdown(cx)
    }
  }

  impl hyper_util::client::legacy::connect::Connection for TurmoilConnection {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
      hyper_util::client::legacy::connect::Connected::new()
    }
  }
}
// mod connector {
//   use std::{future::Future, pin::Pin};

//   use hyper::{
//     client::connect::{Connected, Connection},
//     Uri,
//   };
//   use tokio::io::{AsyncRead, AsyncWrite};
//   use tower::Service;
//   use turmoil::net::TcpStream;

//   type Fut = Pin<Box<dyn Future<Output = Result<TurmoilConnection, std::io::Error>> + Send>>;

//   pub fn connector(
//   ) -> impl Service<Uri, Response = TurmoilConnection, Error = std::io::Error, Future = Fut> + Clone
//   {
//     tower::service_fn(|uri: Uri| {
//       Box::pin(async move {
//         let conn = TcpStream::connect(uri.authority().unwrap().as_str()).await?;
//         Ok::<_, std::io::Error>(TurmoilConnection(conn))
//       }) as Fut
//     })
//   }

//   pub struct TurmoilConnection(turmoil::net::TcpStream);

//   impl AsyncRead for TurmoilConnection {
//     fn poll_read(
//       mut self: std::pin::Pin<&mut Self>,
//       cx: &mut std::task::Context<'_>,
//       buf: &mut tokio::io::ReadBuf<'_>,
//     ) -> std::task::Poll<std::io::Result<()>> {
//       Pin::new(&mut self.0).poll_read(cx, buf)
//     }
//   }

//   impl AsyncWrite for TurmoilConnection {
//     fn poll_write(
//       mut self: Pin<&mut Self>,
//       cx: &mut std::task::Context<'_>,
//       buf: &[u8],
//     ) -> std::task::Poll<Result<usize, std::io::Error>> {
//       Pin::new(&mut self.0).poll_write(cx, buf)
//     }

//     fn poll_flush(
//       mut self: Pin<&mut Self>,
//       cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Result<(), std::io::Error>> {
//       Pin::new(&mut self.0).poll_flush(cx)
//     }

//     fn poll_shutdown(
//       mut self: Pin<&mut Self>,
//       cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Result<(), std::io::Error>> {
//       Pin::new(&mut self.0).poll_shutdown(cx)
//     }
//   }

//   impl Connection for TurmoilConnection {
//     fn connected(&self) -> hyper::client::connect::Connected {
//       Connected::new()
//     }
//   }
// }

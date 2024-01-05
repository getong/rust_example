use futures::{future, prelude::*};
use poem::{listener::TcpListener, Route};
use poem_openapi::OpenApiService;
use std::{net::IpAddr, net::Ipv4Addr, sync::Arc};
use tarpc::{
  server::{self, incoming::Incoming, Channel},
  tokio_serde::formats::Json,
};
use tokio::sync::Mutex;

mod api_rpc;
mod common;
mod web_openapi;

use api_rpc::*;
use common::*;

// --------------- doc begin ------------
// cargo run

// # on other terminal
// cd ../../tarpc_workspace_example/tarpc_service_client_example
// cargo run -- --server-addr 127.0.0.1:12345 --name world

// curl http://localhost:3000/api/hello
// ---------------doc end ------------

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
  let num = Arc::new(Mutex::new(0));
  let num_clone = num.clone();

  _ = start_tarpc(num_clone).await;
  _ = start_poem(num).await;
  match send_hello_msg().await {
    Ok(result) => {
      println!("result: {:?}", result);
      Ok(())
    }
    Err(e) => Err(e),
  }
}

async fn start_tarpc(num_clone: Arc<Mutex<i64>>) -> Result<(), std::io::Error> {
  let server_addr = (IpAddr::V4(Ipv4Addr::LOCALHOST), 12345);
  let mut listener = tarpc::serde_transport::tcp::listen(&server_addr, Json::default).await?;
  listener.config_mut().max_frame_length(usize::MAX);
  tokio::spawn(async move {
    listener
      // Ignore accept errors.
      .filter_map(|r| future::ready(r.ok()))
      .map(server::BaseChannel::with_defaults)
      // Limit channels to 1 per IP.
      .max_channels_per_key(1, |t| t.transport().peer_addr().unwrap().ip())
      // serve is generated by the service attribute. It takes as input any type implementing
      // the generated World trait.
      .map(|channel| {
        let server = Api {
          num: num_clone.clone(),
        };
        channel.execute(server.serve()).for_each(spawn)
      })
      // Max 10 channels.
      .buffer_unordered(10)
      .for_each(|_| async {})
      .await;
  });
  Ok(())
}

async fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
  tokio::spawn(fut);
}

async fn start_poem(num: Arc<Mutex<i64>>) -> Result<(), std::io::Error> {
  let api_service =
    OpenApiService::new(Api { num }, "Hello World", "1.0").server("http://localhost:3000/api");

  let app = Route::new().nest("/api", api_service);

  println!("access http://127.0.0.1:3000/api/hello");

  poem::Server::new(TcpListener::bind("127.0.0.1:3000"))
    .run(app)
    .await
}

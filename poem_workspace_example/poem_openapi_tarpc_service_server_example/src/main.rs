use futures::{future, prelude::*};
use poem::{listener::TcpListener, Route};
use poem_openapi::{param::Query, payload::PlainText, OpenApi, OpenApiService};
use std::{net::IpAddr, net::Ipv4Addr, sync::Arc};
use tarpc::{
  context,
  server::{self, incoming::Incoming, Channel},
  tokio_serde::formats::Json,
};
use tokio::sync::Mutex;

// --------------- doc begin ------------
// cargo run

// # on other terminal
// cd ../../tarpc_workspace_example/tarpc_service_client_example
// cargo run

// curl http://localhost:3000/api/hello
// ---------------doc end ------------

#[derive(Clone)]
struct Api {
  num: Arc<Mutex<i64>>,
}

#[OpenApi]
impl Api {
  #[oai(path = "/hello", method = "get")]
  async fn index(&self, name: Query<Option<String>>) -> PlainText<String> {
    let recv_name = match name.0 {
      Some(name) => name,
      None => "unknown!".to_string(),
    };
    PlainText(format!(
      "hello, {}, the current num is {:?}!\n",
      recv_name,
      self.num.lock().await
    ))
  }
}

#[tarpc::service]
pub trait World {
  /// Returns a greeting for name.
  async fn hello(name: String) -> String;
}

#[tarpc::server]
impl World for Api {
  async fn hello(self, _context_info: context::Context, name: String) -> String {
    let mut num = self.num.lock().await;
    *num += 1;
    format!(
      "Hello, {name}! You are connected from {}, access num is {}",
      name, num
    )
  }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
  let num = Arc::new(Mutex::new(0));
  let num_clone = num.clone();

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
        channel.execute(server.serve())
      })
      // Max 10 channels.
      .buffer_unordered(10)
      .for_each(|_| async {})
      .await
  });

  let api_service =
    OpenApiService::new(Api { num }, "Hello World", "1.0").server("http://localhost:3000/api");

  let app = Route::new().nest("/api", api_service);

  println!("access http://127.0.0.1:3000/api/hellocd");

  poem::Server::new(TcpListener::bind("127.0.0.1:3000"))
    .run(app)
    .await
}

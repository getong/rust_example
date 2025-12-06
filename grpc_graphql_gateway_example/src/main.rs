#![allow(dead_code)]

mod generated;

use std::{net::SocketAddr, pin::Pin, sync::Arc};

use generated::greeter::{
  GetUserRequest, GreetMeta, HelloReply, HelloRequest, UpdateGreetingRequest, UploadAvatarReply,
  UploadAvatarRequest, UploadAvatarsReply, UploadAvatarsRequest, User,
  greeter_server::{Greeter, GreeterServer},
};
use grpc_graphql_gateway::{Gateway, GrpcClient};
use tokio::sync::RwLock;
use tokio_stream::{self as stream, Stream};
use tonic::{Request, Response, Status, transport::Server};

// Descriptor set containing the Greeter service definitions + GraphQL options.
const DESCRIPTORS: &[u8] = include_bytes!("../src/generated/greeter_descriptor.bin");
const GRPC_ADDR: &str = "127.0.0.1:50051";
const GATEWAY_BIND: &str = "0.0.0.0:8000";
const GATEWAY_HTTP: &str = "http://127.0.0.1:8000";
type AnyError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone)]
struct ExampleGreeter {
  greeting: Arc<RwLock<String>>,
}

impl Default for ExampleGreeter {
  fn default() -> Self {
    Self {
      greeting: Arc::new(RwLock::new("Hello".to_string())),
    }
  }
}

#[tonic::async_trait]
impl Greeter for ExampleGreeter {
  type StreamHellosStream =
    Pin<Box<dyn Stream<Item = Result<HelloReply, Status>> + Send + 'static>>;

  async fn say_hello(
    &self,
    request: Request<HelloRequest>,
  ) -> Result<Response<HelloReply>, Status> {
    let name = request.into_inner().name;
    let greeting = self.greeting.read().await.clone();
    let message = format!(
      "{greeting}, {}!",
      if name.is_empty() { "World" } else { &name }
    );
    let reply = HelloReply {
      message,
      meta: Some(GreetMeta {
        correlation_id: "demo-correlation-id".into(),
        from: Some(User {
          id: "demo-user".into(),
          display_name: "GraphQL Greeter".into(),
          trusted: true,
        }),
      }),
    };

    Ok(Response::new(reply))
  }

  async fn update_greeting(
    &self,
    request: Request<UpdateGreetingRequest>,
  ) -> Result<Response<HelloReply>, Status> {
    let payload = request.into_inner();
    {
      let mut greeting = self.greeting.write().await;
      *greeting = payload.greeting.clone();
    }

    let reply = HelloReply {
      message: format!("{}, {}!", payload.greeting, payload.name),
      meta: Some(GreetMeta {
        correlation_id: "demo-correlation-id".into(),
        from: Some(User {
          id: "demo-user".into(),
          display_name: "GraphQL Greeter".into(),
          trusted: true,
        }),
      }),
    };

    Ok(Response::new(reply))
  }

  async fn stream_hellos(
    &self,
    request: Request<HelloRequest>,
  ) -> Result<Response<Self::StreamHellosStream>, Status> {
    let name = request.into_inner().name;
    let greeting = self.greeting.read().await.clone();
    let messages = (1 ..= 3).map(move |i| HelloReply {
      message: format!("{greeting}, {name}! (#{i})"),
      meta: Some(GreetMeta {
        correlation_id: format!("stream-{i}"),
        from: Some(User {
          id: format!("user-{i}"),
          display_name: format!("Greeter {i}"),
          trusted: true,
        }),
      }),
    });

    let stream = stream::iter(messages.map(Ok));
    Ok(Response::new(Box::pin(stream) as Self::StreamHellosStream))
  }

  async fn resolve_user(&self, request: Request<GetUserRequest>) -> Result<Response<User>, Status> {
    let req = request.into_inner();
    let user = User {
      id: req.id.clone(),
      display_name: format!("User {}", req.id),
      trusted: true,
    };

    Ok(Response::new(user))
  }

  async fn upload_avatar(
    &self,
    request: Request<UploadAvatarRequest>,
  ) -> Result<Response<UploadAvatarReply>, Status> {
    let req = request.into_inner();
    let reply = UploadAvatarReply {
      user_id: req.user_id,
      size: req.avatar.len() as u64,
    };

    Ok(Response::new(reply))
  }

  async fn upload_avatars(
    &self,
    request: Request<UploadAvatarsRequest>,
  ) -> Result<Response<UploadAvatarsReply>, Status> {
    let req = request.into_inner();
    let sizes = req
      .avatars
      .iter()
      .map(|a| a.len() as u64)
      .collect::<Vec<u64>>();
    let reply = UploadAvatarsReply {
      user_id: req.user_id,
      sizes,
    };

    Ok(Response::new(reply))
  }
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
  let grpc_server = tokio::spawn(run_grpc_backend(ExampleGreeter::default()));
  let gateway = tokio::spawn(run_graphql_gateway());

  // Surface a couple of ready-to-run GraphQL examples.
  println!("GraphQL Playground  : {GATEWAY_HTTP}/graphql");
  println!(
    "Sample query (curl) : curl -XPOST -H 'content-type: application/json' --data \
     '{{\"query\":\"{{ hello(name:\\\"Rust\\\") {{ message }} }}\"}}' {GATEWAY_HTTP}/graphql"
  );
  println!(
    "Sample mutation     : curl -XPOST -H 'content-type: application/json' --data \
     '{{\"query\":\"mutation {{ updateGreeting(input: {{ name:\\\"Rust\\\", \
     salutation:\\\"Ahoy\\\" }}) {{ message }} }}\"}}' {GATEWAY_HTTP}/graphql"
  );

  tokio::select! {
    res = grpc_server => res??,
    res = gateway => res??,
  };

  Ok(())
}

async fn run_grpc_backend(service: ExampleGreeter) -> Result<(), AnyError> {
  let addr: SocketAddr = GRPC_ADDR.parse()?;
  println!("Demo gRPC backend   : http://{GRPC_ADDR}");

  Server::builder()
    .add_service(GreeterServer::new(service))
    .serve(addr)
    .await?;

  Ok(())
}

async fn run_graphql_gateway() -> Result<(), AnyError> {
  let gateway = Gateway::builder()
    .with_descriptor_set_bytes(DESCRIPTORS)
    .add_grpc_client(
      "greeter.Greeter",
      GrpcClient::builder(format!("http://{GRPC_ADDR}")).connect_lazy()?,
    )
    .build()?;

  let listener = tokio::net::TcpListener::bind(GATEWAY_BIND).await?;
  println!("GraphQL gateway     : {GATEWAY_HTTP}");

  let app = gateway.into_router();
  axum::serve(listener, app).await?;

  Ok(())
}

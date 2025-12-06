use grpc_graphql_gateway::{Gateway, GrpcClient};

// Descriptor set containing the Greeter service definitions + GraphQL options.
const DESCRIPTORS: &[u8] = include_bytes!("../src/generated/greeter_descriptor.bin");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let gateway = Gateway::builder()
    .with_descriptor_set_bytes(DESCRIPTORS)
    .add_grpc_client(
      "greeter.Greeter",
      GrpcClient::builder("http://127.0.0.1:50051").connect_lazy()?,
    )
    .build()?;

  let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();

  tracing::info!("Gateway server listening on 8000 port");

  let app = gateway.into_router();
  axum::serve(listener, app).await?;

  Ok(())
}

use std::error::Error;

use axum_handlebars_example::{dynamic, generic_static, struct_static, trait_static};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  println!("Starting demos");
  let socket_9090 = "127.0.0.1:9090";
  let socket_9091 = "127.0.0.1:9091";
  let socket_9092 = "127.0.0.1:9092";
  let socket_9093 = "127.0.0.1:9093";
  let listener1 = tokio::net::TcpListener::bind(socket_9090).await.unwrap();
  let listener2 = tokio::net::TcpListener::bind(socket_9091).await.unwrap();
  let listener3 = tokio::net::TcpListener::bind(socket_9092).await.unwrap();
  let listener4 = tokio::net::TcpListener::bind(socket_9093).await.unwrap();

  let _ = tokio::join!(
    axum::serve(listener1, struct_static::build_router().into_make_service()),
    axum::serve(
      listener2,
      generic_static::build_router().into_make_service()
    ),
    axum::serve(listener3, trait_static::build_router().into_make_service()),
    axum::serve(listener4, dynamic::build_router().into_make_service()),
  );

  Ok(())
}

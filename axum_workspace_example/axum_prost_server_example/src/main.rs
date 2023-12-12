use axum::{
  routing::{get, post},
  Extension, Router,
};

use axum_extra::protobuf::Protobuf;
use myapp::Todo;
use std::sync::Arc;

pub mod myapp {
  include!("myapp.rs");
}

async fn create_todo_handler(
  Extension(_todo): Extension<Arc<Todo>>,
  Protobuf(todo_new): Protobuf<Todo>,
) -> String {
  println!("Received todo: {:?}", todo_new);
  format!(
    "Created new todo with id {}: {}",
    todo_new.id, todo_new.title
  )
}

#[tokio::main]
async fn main() {
  let router = Router::new()
    .route("/todos", post(create_todo_handler))
    .route("/todos", get(create_todo_handler));

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, router).await.unwrap();
}

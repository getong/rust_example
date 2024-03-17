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
  Extension(todo): Extension<Arc<Todo>>,
  Protobuf(todo_new): Protobuf<Todo>,
) -> String {
  println!("Received todo: {:?}", todo_new);
  println!("todo: {:?}", todo);
  format!(
    "Created new todo with id {}: {}, current todo id: {}, title: {}, completed: {}\n",
    todo_new.id, todo_new.title, todo.id, todo.title, todo.completed
  )
}

// curl http://localhost:3000/todos
#[tokio::main]
async fn main() {
  let todo = Todo {
    id: 1,
    title: "hello".to_owned(),
    completed: false,
  };
  let router = Router::new()
    .route("/todos", post(create_todo_handler))
    .route("/todos", get(create_todo_handler))
    .layer(Extension(Arc::new(todo)));

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, router).await.unwrap();
}
